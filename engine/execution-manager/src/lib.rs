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

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use whistle::{OutboundQueue, TickId};

/// ExecutionManager - the central post-match emission and event distribution service
///
/// This is the only authorized egress point for market events produced by Whistle engines.
/// It ensures that all trade executions, order state changes, and system-level outputs
/// are captured, formatted, and dispatched deterministically.
pub struct ExecutionManager {
    config: ExecManagerConfig,
    metrics: Arc<MetricsCollector>,

    // Core components
    id_allocator: ExecutionIdAllocator,
    normalizer: EventNormalizer,
    dispatcher: EventDispatcher,
    tick_tracker: TickTracker,

    // State tracking
    active_symbols: HashMap<u32, SymbolInfo>,
    shutdown_manager: ShutdownManager,

    // Performance tracking
    start_time: Instant,
    total_events_processed: u64,
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
    pub fn new(config: ExecManagerConfig) -> Self {
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
            active_symbols: HashMap::new(),
            shutdown_manager,
            start_time: Instant::now(),
            total_events_processed: 0,
        }
    }

    /// Register a symbol with the ExecutionManager
    ///
    /// This must be called before any events for the symbol are processed.
    pub fn register_symbol(&mut self, symbol_id: u32) {
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
    }

    /// Deregister a symbol from the ExecutionManager
    ///
    /// This should be called when a Whistle engine is shut down.
    pub fn deregister_symbol(&mut self, symbol_id: u32) {
        self.active_symbols.remove(&symbol_id);
        self.tick_tracker.deregister_symbol(symbol_id);

        self.metrics.symbols_active.set(self.active_symbols.len() as u64);
        self.metrics.symbols_deregistered_total.inc();
    }

    /// Process events from a Whistle engine's OutboundQueue
    ///
    /// This is the main ingestion method that consumes events from Whistle
    /// and processes them through the normalization and dispatch pipeline.
    pub fn process_events(
        &mut self,
        symbol_id: u32,
        queue: &OutboundQueue,
    ) -> Result<(), ExecutionError> {
        // Verify symbol is registered
        if !self.active_symbols.contains_key(&symbol_id) {
            return Err(ExecutionError::UnregisteredSymbol(symbol_id));
        }

        // Drain events from the queue
        let events = queue.drain(self.config.batch_size);
        if events.is_empty() {
            return Ok(());
        }

        let start_time = Instant::now();
        let mut processed_count = 0;

        // Process each event through the pipeline
        for event in events {
            // Normalize the event
            let normalized = self.normalizer.normalize(event, &mut self.id_allocator)?;

            // Update symbol tracking
            if let Some(symbol_info) = self.active_symbols.get_mut(&symbol_id) {
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

        // Update metrics
        let processing_time = start_time.elapsed();
        self.metrics.events_processed_total.add(processed_count);
        self.metrics.processing_latency.record(processing_time.as_nanos() as u64);
        self.total_events_processed += processed_count;

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
            flushed_symbols: self.active_symbols.keys().cloned().collect(),
            timestamp: start_time,
            events_processed: self.total_events_processed,
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

    /// Get statistics about the current state
    pub fn get_stats(&self) -> ExecutionStats {
        ExecutionStats {
            active_symbols: self.active_symbols.len(),
            total_events_processed: self.total_events_processed,
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
}

/// Statistics about ExecutionManager performance
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    pub active_symbols: usize,
    pub total_events_processed: u64,
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

    #[test]
    fn test_execution_manager_creation() {
        let config = create_test_config();
        let manager = ExecutionManager::new(config);

        assert_eq!(manager.active_symbols.len(), 0);
        assert_eq!(manager.total_events_processed, 0);
    }

    #[test]
    fn test_symbol_registration() {
        let config = create_test_config();
        let mut manager = ExecutionManager::new(config);

        manager.register_symbol(1);
        assert_eq!(manager.active_symbols.len(), 1);
        assert!(manager.active_symbols.contains_key(&1));

        manager.register_symbol(2);
        assert_eq!(manager.active_symbols.len(), 2);

        manager.deregister_symbol(1);
        assert_eq!(manager.active_symbols.len(), 1);
        assert!(!manager.active_symbols.contains_key(&1));
    }

    #[test]
    fn test_unregistered_symbol_error() {
        let config = create_test_config();
        let mut manager = ExecutionManager::new(config);

        // Create a mock queue (we'll need to implement this properly)
        // For now, this test verifies the error handling logic
        let result = manager.process_events(1, &OutboundQueue::with_default_capacity());
        assert!(matches!(result, Err(ExecutionError::UnregisteredSymbol(1))));
    }
}
