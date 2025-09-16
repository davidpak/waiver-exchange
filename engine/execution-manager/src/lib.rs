// ExecutionManager - post-match event distribution and fanout service

mod analytics_converter;
mod config;
mod dispatch;
mod event;
mod id_allocator;
mod ingestion;
mod metrics;
mod normalization;
mod shutdown;
mod tick_tracker;

pub use config::ShutdownConfig;
pub use config::{BackpressureConfig, ExecManagerConfig, FanoutConfig};
pub use dispatch::{DispatchStats, EventDispatcher, FanoutDestination};
pub use event::{
    BookDelta, DispatchEvent, ExecutionReport, OrderCancelled, SystemLog, TickBoundaryEvent,
    TradeEvent,
};
pub use id_allocator::{ExecutionId, ExecutionIdAllocator};
pub use ingestion::{EventIngestion, IngestionStats};
pub use metrics::{ExecutionMetrics, MetricsCollector};
pub use normalization::{EventNormalizer, NormalizedEvent};
pub use shutdown::ShutdownManager;
pub use tick_tracker::{TickBoundaryStats, TickTracker};

use dashmap::DashMap;
use persistence::{PersistenceBackend, WalOperation};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use whistle::{OutboundQueue, TickId};
use account_service::{AccountService, trade::TradeDetails, position::TradeSide};

/// ExecutionManager - the central post-match emission and event distribution service
///
/// This is the only authorized egress point for market events produced by Whistle engines.
/// It ensures that all trade executions, order state changes, and system-level outputs
/// are captured, formatted, and dispatched deterministically.
pub struct ExecutionManager {
    #[allow(dead_code)]
    config: ExecManagerConfig,
    metrics: Arc<MetricsCollector>,

    // Core components
    id_allocator: ExecutionIdAllocator,
    normalizer: EventNormalizer,
    dispatcher: EventDispatcher,
    tick_tracker: TickTracker,

    // Persistence integration
    persistence: Arc<dyn PersistenceBackend>,

    // Account service for trade settlement
    account_service: Arc<AccountService>,

    // State tracking (lock-free for hot path compatibility)
    active_symbols: DashMap<u32, SymbolInfo>,
    shutdown_manager: ShutdownManager,

    // Performance tracking (atomic for lock-free access)
    start_time: Instant,
    total_events_processed: AtomicU64,
    total_orders: AtomicU64,
    total_trades: AtomicU64,
    total_volume: AtomicU64,
}

/// Information about an active symbol
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub symbol_id: u32,
    pub registered_at: Instant,
    pub last_tick_seen: Option<TickId>,
    pub events_processed: u64,
}

impl ExecutionManager {
    /// Create a new ExecutionManager with the specified configuration
    pub fn new(config: ExecManagerConfig, account_service: Arc<AccountService>) -> Self {
        let metrics = Arc::new(MetricsCollector::new());
        let id_allocator = ExecutionIdAllocator::new(config.execution_id_config.clone());
        let normalizer = EventNormalizer::new(config.normalization_config.clone());
        let dispatcher = EventDispatcher::new(config.fanout_config.clone(), metrics.clone());
        let tick_tracker = TickTracker::new(config.tick_tracking_config.clone());
        let shutdown_manager = ShutdownManager::new(config.shutdown_config.clone());

        Self {
            config,
            metrics,
            id_allocator,
            normalizer,
            dispatcher,
            tick_tracker,
            persistence: Arc::new(persistence::InMemoryPersistence::with_default_config()),
            account_service,
            active_symbols: DashMap::new(),
            shutdown_manager,
            start_time: Instant::now(),
            total_events_processed: AtomicU64::new(0),
            total_orders: AtomicU64::new(0),
            total_trades: AtomicU64::new(0),
            total_volume: AtomicU64::new(0),
        }
    }

    /// Create a new ExecutionManager with persistence integration
    pub fn new_with_persistence(
        config: ExecManagerConfig,
        persistence: Arc<dyn PersistenceBackend>,
        account_service: Arc<AccountService>,
    ) -> Self {
        let metrics = Arc::new(MetricsCollector::new());
        let id_allocator = ExecutionIdAllocator::new(config.execution_id_config.clone());
        let normalizer = EventNormalizer::new(config.normalization_config.clone());
        let dispatcher = EventDispatcher::new(config.fanout_config.clone(), metrics.clone());
        let tick_tracker = TickTracker::new(config.tick_tracking_config.clone());
        let shutdown_manager = ShutdownManager::new(config.shutdown_config.clone());

        Self {
            config,
            metrics,
            id_allocator,
            normalizer,
            dispatcher,
            tick_tracker,
            persistence,
            account_service,
            active_symbols: DashMap::new(),
            shutdown_manager,
            start_time: Instant::now(),
            total_events_processed: AtomicU64::new(0),
            total_orders: AtomicU64::new(0),
            total_trades: AtomicU64::new(0),
            total_volume: AtomicU64::new(0),
        }
    }

    /// Register a symbol with the ExecutionManager
    ///
    /// This must be called before any events for the symbol are processed.
    pub fn register_symbol(&self, symbol_id: u32) {
        let symbol_info = SymbolInfo {
            symbol_id,
            registered_at: Instant::now(),
            last_tick_seen: None,
            events_processed: 0,
        };

        self.active_symbols.insert(symbol_id, symbol_info);
        self.tick_tracker.register_symbol(symbol_id);

        self.metrics.symbols_active.set(self.active_symbols.len() as u64);
        self.metrics.symbols_registered_total.inc();

        tracing::info!(
            "ExecutionManager registered symbol {} (total active: {})",
            symbol_id,
            self.active_symbols.len()
        );
    }

    /// Deregister a symbol from the ExecutionManager
    ///
    /// This should be called when a Whistle engine is shut down.
    pub fn deregister_symbol(&self, symbol_id: u32) {
        self.active_symbols.remove(&symbol_id);
        self.tick_tracker.deregister_symbol(symbol_id);

        self.metrics.symbols_active.set(self.active_symbols.len() as u64);
        self.metrics.symbols_deregistered_total.inc();
    }

    /// Process events from a Whistle engine's OutboundQueue
    ///
    /// This is the main ingestion method that consumes events from Whistle
    /// and processes them through the normalization and dispatch pipeline.
    ///
    /// This method is designed to work with Arc<ExecutionManager> for hot path compatibility.
    pub async fn process_events(
        &self,
        symbol_id: u32,
        queue: &OutboundQueue,
    ) -> Result<(), ExecutionError> {
        // Verify symbol is registered
        if !self.active_symbols.contains_key(&symbol_id) {
            return Err(ExecutionError::UnregisteredSymbol(symbol_id));
        }

        // Drain ALL events from the queue to prevent overflow
        // We need to drain the entire queue, not just a batch, to prevent OutboundQueue overflow
        // Use a very large number to drain all available events
        let events = queue.drain(usize::MAX);

        if events.is_empty() {
            // Only log occasionally to avoid spam
            if symbol_id == 889
                && std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    % 10
                    == 0
            {
                tracing::debug!("ExecutionManager: No events to process for symbol {}", symbol_id);
            }
            return Ok(());
        }

        tracing::info!(
            "ExecutionManager processing {} events for symbol {}",
            events.len(),
            symbol_id
        );

        let start_time = Instant::now();
        let mut processed_count = 0;

        // Process each event through the pipeline
        for event in events {
            // Normalize the event
            let normalized = self.normalizer.normalize(event, &self.id_allocator)?;

            // Update metrics based on event type
            match &normalized {
                DispatchEvent::OrderSubmitted(_) => {
                    self.total_orders.fetch_add(1, Ordering::Relaxed);
                    tracing::info!(
                        "Incremented total_orders to {}",
                        self.total_orders.load(Ordering::Relaxed)
                    );
                }
                DispatchEvent::TradeEvent(trade) => {
                    self.total_trades.fetch_add(1, Ordering::Relaxed);
                    self.total_volume.fetch_add(trade.quantity as u64, Ordering::Relaxed);
                    tracing::info!(
                        "Incremented total_trades to {}, total_volume to {}",
                        self.total_trades.load(Ordering::Relaxed),
                        self.total_volume.load(Ordering::Relaxed)
                    );

                    // Settle the trade with AccountService
                    if let Err(e) = self.settle_trade(trade).await {
                        tracing::error!("Failed to settle trade: {}", e);
                        return Err(e);
                    }
                }
                _ => {}
            }

            // Write to WAL for persistence
            if let Err(e) = self.write_event_to_wal(&normalized, symbol_id) {
                tracing::warn!("Failed to write event to WAL: {}", e);
                // Continue processing even if WAL write fails
            } else {
                tracing::debug!("Successfully wrote event to WAL: {:?}", normalized);
            }

            // Update symbol tracking (lock-free)
            if let Some(mut symbol_info) = self.active_symbols.get_mut(&symbol_id) {
                if let Some(tick) = normalized.logical_timestamp() {
                    symbol_info.last_tick_seen = Some(tick);
                }
                symbol_info.events_processed += 1;
            }

            // Dispatch to downstream systems
            self.dispatcher.dispatch(normalized.clone()).map_err(ExecutionError::DispatchFailed)?;

            // Update tick tracking
            self.tick_tracker
                .process_event(&normalized)
                .map_err(ExecutionError::TickTrackingFailed)?;

            processed_count += 1;
        }

        // Update metrics (lock-free)
        let processing_time = start_time.elapsed();
        self.metrics.events_processed_total.add(processed_count);
        self.metrics.processing_latency.record(processing_time.as_nanos() as u64);
        self.total_events_processed.fetch_add(processed_count, Ordering::Relaxed);

        Ok(())
    }

    /// Check if a tick is ready to be flushed
    ///
    /// Returns true if all registered symbols have completed the specified tick.
    pub fn is_tick_ready(&self, tick_id: TickId) -> bool {
        self.tick_tracker.is_tick_ready(tick_id)
    }

    /// Flush a completed tick to downstream systems
    ///
    /// This emits a TickBoundaryEvent and triggers any tick-based processing.
    pub fn flush_tick(&mut self, tick_id: TickId) -> Result<(), ExecutionError> {
        if !self.is_tick_ready(tick_id) {
            return Err(ExecutionError::TickNotReady(tick_id));
        }

        let start_time = Instant::now();

        // Create tick boundary event
        let tick_boundary = TickBoundaryEvent {
            tick: tick_id,
            flushed_symbols: self.active_symbols.iter().map(|entry| *entry.key()).collect(),
            timestamp: start_time,
            events_processed: self.total_events_processed.load(Ordering::Relaxed),
        };

        // Dispatch tick boundary event
        let dispatch_event = DispatchEvent::TickBoundary(tick_boundary);
        self.dispatcher
            .dispatch_tick_boundary(dispatch_event)
            .map_err(ExecutionError::DispatchFailed)?;

        // Update metrics
        let flush_time = start_time.elapsed();
        self.metrics.tick_flush_latency.record(flush_time.as_nanos() as u64);
        self.metrics.ticks_flushed_total.inc();

        Ok(())
    }

    /// Set up analytics integration
    pub fn set_analytics_sender(
        &mut self,
        sender: tokio::sync::mpsc::UnboundedSender<analytics_engine::analytics::AnalyticsEvent>,
    ) {
        self.dispatcher.set_analytics_sender(sender);
    }

    /// Get current metrics for monitoring
    pub fn metrics(&self) -> &MetricsCollector {
        &self.metrics
    }

    /// Settle a trade by updating account balances and positions
    async fn settle_trade(&self, trade_event: &TradeEvent) -> Result<(), ExecutionError> {
        use account_service::Balance;
        use whistle::Side;

        // Convert Whistle types to AccountService types
        let price = Balance::from_cents(trade_event.price as i64);
        let quantity = Balance::from_basis_points(trade_event.quantity as i64);
        let symbol_id = trade_event.symbol as i64;

        // Determine buy and sell sides
        let (buy_order_id, sell_order_id) = match trade_event.aggressor_side {
            Side::Buy => (trade_event.taker_order_id, trade_event.maker_order_id),
            Side::Sell => (trade_event.maker_order_id, trade_event.taker_order_id),
        };

        // Get account IDs from order IDs (we'll need to implement this mapping)
        // For now, we'll use a simple mapping - in production this would come from order metadata
        let buy_account_id = self.get_account_id_from_order_id(buy_order_id).await?;
        let sell_account_id = self.get_account_id_from_order_id(sell_order_id).await?;

        // Create trade details for both sides
        let buy_trade = TradeDetails {
            account_id: buy_account_id,
            symbol_id,
            side: TradeSide::Buy,
            quantity,
            price,
            order_id: buy_order_id as i64,
        };

        let sell_trade = TradeDetails {
            account_id: sell_account_id,
            symbol_id,
            side: TradeSide::Sell,
            quantity,
            price,
            order_id: sell_order_id as i64,
        };

        // Settle both trades
        self.account_service.settle_trade(&buy_trade).await
            .map_err(|e| ExecutionError::SettlementFailed(format!("Buy trade settlement failed: {}", e)))?;

        self.account_service.settle_trade(&sell_trade).await
            .map_err(|e| ExecutionError::SettlementFailed(format!("Sell trade settlement failed: {}", e)))?;

        tracing::info!(
            "Successfully settled trade: {} shares of symbol {} at price {} between accounts {} and {}",
            trade_event.quantity,
            trade_event.symbol,
            trade_event.price,
            buy_account_id,
            sell_account_id
        );

        Ok(())
    }

    /// Get account ID from order ID (placeholder implementation)
    /// 
    /// In a real system, this would query the order metadata or maintain a mapping.
    /// For now, we'll use a simple hash-based approach for testing.
    async fn get_account_id_from_order_id(&self, order_id: u64) -> Result<i64, ExecutionError> {
        // Simple hash-based mapping for testing
        // In production, this would be a proper database lookup
        let account_id = (order_id % 1000) as i64 + 1; // Map to account IDs 1-1000
        Ok(account_id)
    }

    /// Get statistics about the current state
    pub fn get_stats(&self) -> ExecutionStats {
        ExecutionStats {
            active_symbols: self.active_symbols.len(),
            total_events_processed: self.total_events_processed.load(Ordering::Relaxed),
            total_orders: self.total_orders.load(Ordering::Relaxed),
            total_trades: self.total_trades.load(Ordering::Relaxed),
            total_volume: self.total_volume.load(Ordering::Relaxed),
            uptime: self.start_time.elapsed(),
            queue_stats: self.dispatcher.get_queue_stats(),
            tick_stats: self.tick_tracker.get_stats(),
        }
    }

    /// Shutdown the ExecutionManager gracefully
    ///
    /// This flushes any remaining events and closes all downstream connections.
    pub fn shutdown(&mut self) -> Result<(), ExecutionError> {
        self.shutdown_manager.initiate_shutdown();

        // Flush any pending ticks
        let current_tick = self.tick_tracker.get_current_tick();
        if let Some(tick) = current_tick {
            if self.is_tick_ready(tick) {
                self.flush_tick(tick)?;
            }
        }

        // Shutdown dispatcher
        self.dispatcher.shutdown().map_err(ExecutionError::ShutdownFailed)?;

        Ok(())
    }

    /// Write a normalized event to WAL for persistence
    fn write_event_to_wal(
        &self,
        event: &NormalizedEvent,
        symbol_id: u32,
    ) -> Result<(), ExecutionError> {
        use chrono::Utc;

        // Convert NormalizedEvent to WalOperation
        let wal_operation = match event {
            NormalizedEvent::ExecutionReport(report) => WalOperation::Trade {
                symbol_id,
                buy_order_id: if report.side == whistle::Side::Buy { report.order_id } else { 0 },
                sell_order_id: if report.side == whistle::Side::Sell { report.order_id } else { 0 },
                price: report.price as u64,
                quantity: report.quantity as u64,
                timestamp: Utc::now(),
            },
            NormalizedEvent::TradeEvent(trade) => {
                // Determine which order is buy and which is sell based on aggressor side
                let (buy_order_id, sell_order_id) = match trade.aggressor_side {
                    whistle::Side::Buy => (trade.taker_order_id, trade.maker_order_id),
                    whistle::Side::Sell => (trade.maker_order_id, trade.taker_order_id),
                };

                WalOperation::Trade {
                    symbol_id,
                    buy_order_id,
                    sell_order_id,
                    price: trade.price as u64,
                    quantity: trade.quantity as u64,
                    timestamp: Utc::now(),
                }
            }
            NormalizedEvent::OrderSubmitted(submitted) => {
                // Convert order type from u8 to string
                let order_type_str = match submitted.order_type {
                    0 => "limit",
                    1 => "market",
                    2 => "ioc",
                    3 => "post_only",
                    _ => "unknown",
                };

                // Convert side from enum to string
                let side_str = match submitted.side {
                    whistle::Side::Buy => "buy",
                    whistle::Side::Sell => "sell",
                };

                WalOperation::SubmitOrder {
                    symbol_id,
                    account_id: submitted.account_id,
                    side: side_str.to_string(),
                    order_type: order_type_str.to_string(),
                    price: submitted.price.map(|p| p as u64), // Convert u32 to u64
                    quantity: submitted.quantity,
                    order_id: submitted.order_id,
                }
            }
            NormalizedEvent::OrderCancelled(cancelled) => {
                WalOperation::CancelOrder {
                    symbol_id,
                    order_id: cancelled.order_id,
                    account_id: 0, // TODO: Get account_id from order
                }
            }
            NormalizedEvent::BookDelta(_delta) => {
                // BookDelta represents order book state changes
                // We'll log this as a checkpoint to track order book evolution
                WalOperation::Checkpoint {
                    tick: event.logical_timestamp().unwrap_or(0),
                    timestamp: Utc::now(),
                }
            }
            NormalizedEvent::TickBoundary(boundary) => {
                WalOperation::Checkpoint { tick: boundary.tick, timestamp: Utc::now() }
            }
            NormalizedEvent::SystemLog(_log) => {
                // System logs are important for debugging and audit trails
                WalOperation::Checkpoint {
                    tick: event.logical_timestamp().unwrap_or(0),
                    timestamp: Utc::now(),
                }
            }
        };

        // Write to WAL (this is async, but we're in a sync context)
        // We'll need to handle this properly with a runtime or make the method async
        // For now, we'll use a blocking approach
        let persistence = self.persistence.clone();
        let operation = wal_operation.clone();

        // Use tokio::task::block_in_place for async operations in sync context
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { persistence.write_wal_entry(operation).await })
        })
        .map_err(|e| ExecutionError::PersistenceFailed(e.to_string()))?;

        Ok(())
    }
}

/// Statistics about ExecutionManager performance
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    pub active_symbols: usize,
    pub total_events_processed: u64,
    pub total_orders: u64,
    pub total_trades: u64,
    pub total_volume: u64,
    pub uptime: Duration,
    pub queue_stats: DispatchStats,
    pub tick_stats: TickBoundaryStats,
}

/// Errors that can occur during ExecutionManager operations
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Symbol {0} is not registered")]
    UnregisteredSymbol(u32),

    #[error("Tick {0} is not ready for flushing")]
    TickNotReady(TickId),

    #[error("Normalization failed: {0}")]
    NormalizationFailed(String),

    #[error("Dispatch failed: {0}")]
    DispatchFailed(String),

    #[error("Tick tracking failed: {0}")]
    TickTrackingFailed(String),

    #[error("Shutdown failed: {0}")]
    ShutdownFailed(String),

    #[error("Persistence failed: {0}")]
    PersistenceFailed(String),

    #[error("Trade settlement failed: {0}")]
    SettlementFailed(String),
}

impl From<String> for ExecutionError {
    fn from(err: String) -> Self {
        ExecutionError::NormalizationFailed(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Removed unused whistle imports

    fn create_test_config() -> ExecManagerConfig {
        ExecManagerConfig {
            batch_size: 1000,
            execution_id_config: Default::default(),
            normalization_config: Default::default(),
            fanout_config: Default::default(),
            tick_tracking_config: Default::default(),
            shutdown_config: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_execution_manager_creation() {
        let config = create_test_config();
        // Create a mock AccountService for testing
        let account_service = Arc::new(account_service::AccountService::new(
            account_service::AccountServiceConfig::from_env().unwrap_or_else(|_| {
                account_service::AccountServiceConfig {
                    database: account_service::DatabaseConfig {
                        url: "postgresql://test".to_string(),
                        max_connections: 10,
                        min_connections: 1,
                    },
                    redis: account_service::RedisConfig {
                        url: "redis://test".to_string(),
                    },
                    oauth: account_service::OAuthConfig {
                        client_id: "test".to_string(),
                        client_secret: "test".to_string(),
                        auth_url: "test".to_string(),
                        token_url: "test".to_string(),
                        redirect_url: "test".to_string(),
                    },
                    sleeper: account_service::SleeperConfig {
                        api_base_url: "test".to_string(),
                    },
                    fantasy_points_conversion_rate: 10,
                    reservation_expiry_days: 7,
                    cache_ttl_seconds: 5,
                }
            })
        ).await.unwrap());
        
        let manager = ExecutionManager::new(config, account_service);

        assert_eq!(manager.active_symbols.len(), 0);
        assert_eq!(manager.total_events_processed.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_symbol_registration() {
        let config = create_test_config();
        // Create a mock AccountService for testing
        let account_service = Arc::new(account_service::AccountService::new(
            account_service::AccountServiceConfig::from_env().unwrap_or_else(|_| {
                account_service::AccountServiceConfig {
                    database: account_service::DatabaseConfig {
                        url: "postgresql://test".to_string(),
                        max_connections: 10,
                        min_connections: 1,
                    },
                    redis: account_service::RedisConfig {
                        url: "redis://test".to_string(),
                    },
                    oauth: account_service::OAuthConfig {
                        client_id: "test".to_string(),
                        client_secret: "test".to_string(),
                        auth_url: "test".to_string(),
                        token_url: "test".to_string(),
                        redirect_url: "test".to_string(),
                    },
                    sleeper: account_service::SleeperConfig {
                        api_base_url: "test".to_string(),
                    },
                    fantasy_points_conversion_rate: 10,
                    reservation_expiry_days: 7,
                    cache_ttl_seconds: 5,
                }
            })
        ).await.unwrap());
        
        let manager = ExecutionManager::new(config, account_service);

        manager.register_symbol(1);
        assert_eq!(manager.active_symbols.len(), 1);
        assert!(manager.active_symbols.contains_key(&1));

        manager.register_symbol(2);
        assert_eq!(manager.active_symbols.len(), 2);

        manager.deregister_symbol(1);
        assert_eq!(manager.active_symbols.len(), 1);
        assert!(!manager.active_symbols.contains_key(&1));
    }

    #[tokio::test]
    async fn test_unregistered_symbol_error() {
        let config = create_test_config();
        // Create a mock AccountService for testing
        let account_service = Arc::new(account_service::AccountService::new(
            account_service::AccountServiceConfig::from_env().unwrap_or_else(|_| {
                account_service::AccountServiceConfig {
                    database: account_service::DatabaseConfig {
                        url: "postgresql://test".to_string(),
                        max_connections: 10,
                        min_connections: 1,
                    },
                    redis: account_service::RedisConfig {
                        url: "redis://test".to_string(),
                    },
                    oauth: account_service::OAuthConfig {
                        client_id: "test".to_string(),
                        client_secret: "test".to_string(),
                        auth_url: "test".to_string(),
                        token_url: "test".to_string(),
                        redirect_url: "test".to_string(),
                    },
                    sleeper: account_service::SleeperConfig {
                        api_base_url: "test".to_string(),
                    },
                    fantasy_points_conversion_rate: 10,
                    reservation_expiry_days: 7,
                    cache_ttl_seconds: 5,
                }
            })
        ).await.unwrap());
        
        let manager = ExecutionManager::new(config, account_service);

        // Create a mock queue (we'll need to implement this properly)
        // For now, this test verifies the error handling logic
        let result = manager.process_events(1, &OutboundQueue::with_default_capacity()).await;
        assert!(matches!(result, Err(ExecutionError::UnregisteredSymbol(1))));
    }
}
