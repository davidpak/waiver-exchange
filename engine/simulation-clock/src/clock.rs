//! Core SimulationClock implementation

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use futures::future::join_all;
use tokio::sync::Mutex;

use execution_manager::ExecutionManager;
use order_router::OrderRouter;
use persistence::PersistenceBackend;
use symbol_coordinator::{SymbolCoordinator, SymbolCoordinatorApi, SymbolId};
use whistle::TickId;

use crate::config::{ClockConfig, ErrorRecovery, SymbolOrdering};
use crate::error::ClockError;
use crate::metrics::{ClockMetrics, MetricsCollector};

/// The SimulationClock - the system heartbeat that drives all logical time progression
pub struct SimulationClock {
    // Core state
    current_tick: AtomicU64,
    is_running: AtomicBool,

    // Symbol management
    active_symbols: Arc<RwLock<BTreeMap<SymbolId, u32>>>, // For now, just track symbol IDs
    symbol_coordinator: Arc<SymbolCoordinator>,

    // System integration
    execution_manager: Arc<ExecutionManager>,
    #[allow(dead_code)]
    order_router: Arc<OrderRouter>,
    persistence: Arc<dyn PersistenceBackend>,

    // Configuration
    config: ClockConfig,

    // Metrics
    metrics_collector: Arc<MetricsCollector>,

    // Error tracking
    symbol_retry_counts: Arc<RwLock<BTreeMap<SymbolId, u32>>>,
    eviction_queue: Arc<Mutex<Vec<SymbolId>>>,
}

impl SimulationClock {
    /// Create a new SimulationClock
    pub fn new(
        symbol_coordinator: Arc<SymbolCoordinator>,
        execution_manager: Arc<ExecutionManager>,
        order_router: Arc<OrderRouter>,
        persistence: Arc<dyn PersistenceBackend>,
        config: ClockConfig,
    ) -> Result<Self, ClockError> {
        let metrics_collector = Arc::new(MetricsCollector::new(1000)); // Keep 1000 tick history

        Ok(Self {
            current_tick: AtomicU64::new(0),
            is_running: AtomicBool::new(false),
            active_symbols: Arc::new(RwLock::new(BTreeMap::new())),
            symbol_coordinator,
            execution_manager,
            order_router,
            persistence,
            config,
            metrics_collector,
            symbol_retry_counts: Arc::new(RwLock::new(BTreeMap::new())),
            eviction_queue: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Start the SimulationClock main loop (blocks forever)
    pub async fn run_clock_loop(&self) -> Result<(), ClockError> {
        if self.is_running.swap(true, Ordering::SeqCst) {
            return Err(ClockError::ClockAlreadyRunning);
        }

        tracing::info!("Starting SimulationClock main loop");

        // Register all active symbols from SymbolCoordinator
        self.register_existing_symbols()?;

        let mut last_metrics_emission = Instant::now();
        let mut last_health_check = Instant::now();
        let mut last_snapshot_tick = 0u64;

        while self.is_running.load(Ordering::Relaxed) {
            let tick_start = Instant::now();
            let current_tick = self.current_tick.fetch_add(1, Ordering::SeqCst);

            // Update SymbolCoordinator with current tick
            self.symbol_coordinator.update_current_tick(current_tick);

            // Process all active symbols concurrently
            let (_symbols_processed, _symbol_failures) =
                self.process_tick_concurrent(current_tick).await?;

            // Check for newly activated symbols
            self.check_for_new_symbols()?;

            // Process evictions at tick boundary
            self.process_evictions().await?;

            // Create snapshot if interval has passed
            if current_tick - last_snapshot_tick >= self.config.snapshot_interval_ticks {
                if let Err(e) = self.create_snapshot(current_tick).await {
                    tracing::error!("Failed to create snapshot at tick {}: {:?}", current_tick, e);
                } else {
                    tracing::info!("Created snapshot at tick {}", current_tick);
                    last_snapshot_tick = current_tick;
                }
            }

            // Emit metrics if interval has passed
            if last_metrics_emission.elapsed() >= self.config.metrics_interval() {
                self.emit_metrics(current_tick, tick_start.elapsed())?;
                last_metrics_emission = Instant::now();
            }

            // Health check if interval has passed
            if last_health_check.elapsed() >= self.config.health_check_interval() {
                self.perform_health_check()?;
                last_health_check = Instant::now();
            }

            // Wait for next tick (precise timing)
            self.wait_for_next_tick(tick_start)?;
        }

        tracing::info!("SimulationClock main loop stopped");
        Ok(())
    }

    /// Stop the SimulationClock
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::Relaxed);
    }

    /// Register a symbol with the SimulationClock
    pub fn register_symbol(&self, symbol_id: SymbolId) -> Result<(), ClockError> {
        let mut symbols = self
            .active_symbols
            .write()
            .map_err(|_| ClockError::Internal("Failed to acquire symbols lock".to_string()))?;

        if symbols.contains_key(&symbol_id) {
            return Err(ClockError::SymbolAlreadyRegistered { symbol_id });
        }

        // For now, just track the symbol ID
        // TODO: Integrate with SymbolCoordinator to get actual handles
        symbols.insert(symbol_id, 0); // Placeholder value
        self.metrics_collector.update_active_symbols(symbols.len() as u32);

        tracing::info!("Registered symbol {} with SimulationClock", symbol_id);
        Ok(())
    }

    /// Unregister a symbol from the SimulationClock
    pub fn unregister_symbol(&self, symbol_id: SymbolId) -> Result<(), ClockError> {
        let mut symbols = self
            .active_symbols
            .write()
            .map_err(|_| ClockError::Internal("Failed to acquire symbols lock".to_string()))?;

        if symbols.remove(&symbol_id).is_some() {
            self.metrics_collector.update_active_symbols(symbols.len() as u32);
            tracing::info!("Unregistered symbol {} from SimulationClock", symbol_id);
        }

        Ok(())
    }

    /// Get current tick
    pub fn get_current_tick(&self) -> TickId {
        self.current_tick.load(Ordering::Relaxed)
    }

    /// Check if clock is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Check if a symbol is active
    pub fn is_symbol_active(&self, symbol_id: SymbolId) -> bool {
        let symbols = self.active_symbols.read().unwrap();
        symbols.contains_key(&symbol_id)
    }

    /// Get current metrics
    pub fn get_metrics(&self) -> ClockMetrics {
        self.metrics_collector.get_metrics()
    }

    /// Register existing symbols from SymbolCoordinator
    fn register_existing_symbols(&self) -> Result<(), ClockError> {
        let active_symbols = self.symbol_coordinator.get_active_symbol_ids();

        for symbol_id in active_symbols {
            self.register_symbol(symbol_id)?;
        }

        tracing::info!(
            "Registered {} existing symbols with SimulationClock",
            self.active_symbols.read().unwrap().len()
        );
        Ok(())
    }

    /// Check for newly activated symbols and register them
    fn check_for_new_symbols(&self) -> Result<(), ClockError> {
        let active_symbols = self.symbol_coordinator.get_active_symbol_ids();
        let mut registered_symbols = self
            .active_symbols
            .write()
            .map_err(|_| ClockError::Internal("Failed to acquire symbols lock".to_string()))?;

        for symbol_id in active_symbols {
            if let std::collections::btree_map::Entry::Vacant(e) =
                registered_symbols.entry(symbol_id)
            {
                // New symbol activated, register it
                e.insert(0); // Placeholder value
                self.metrics_collector.update_active_symbols(registered_symbols.len() as u32);
                tracing::info!(
                    "Discovered and registered new symbol {} with SimulationClock",
                    symbol_id
                );
            }
        }

        Ok(())
    }

    /// Process all symbols concurrently for current tick
    async fn process_tick_concurrent(&self, tick: TickId) -> Result<(u32, u32), ClockError> {
        let symbol_ids = self.get_active_symbols();
        let symbol_count = symbol_ids.len() as u32;

        if symbol_count == 0 {
            return Ok((0, 0));
        }

        // Debug: Show which symbols are being processed (only when there are multiple symbols or first time)
        if !symbol_ids.is_empty() && (symbol_count > 1 || tick % 1000 == 0) {
            tracing::info!(
                "SimulationClock processing {} symbols at tick {}: {:?}",
                symbol_count,
                tick,
                symbol_ids
            );
        }

        // Process symbols in parallel
        let futures: Vec<_> = symbol_ids
            .into_iter()
            .map(|symbol_id| self.process_symbol_tick(symbol_id, tick))
            .collect();

        // Wait for all symbols to complete
        let results = join_all(futures).await;

        // Count successes and failures
        let mut failures = 0;
        for result in results {
            if let Err(symbol_id) = result {
                self.handle_symbol_failure(symbol_id)?;
                failures += 1;
            }
        }

        let successes = symbol_count - failures;
        Ok((successes, failures))
    }

    /// Process a single symbol tick
    async fn process_symbol_tick(&self, symbol_id: SymbolId, tick: TickId) -> Result<(), SymbolId> {
        let start_time = Instant::now();

        // Call SymbolCoordinator to process the tick
        match self.symbol_coordinator.process_symbol_tick_concurrent(symbol_id, tick) {
            Ok(_events) => {
                // Get the Whistle engine's OutboundQueue and process events through ExecutionManager
                match self.symbol_coordinator.get_outbound_queue(symbol_id) {
                    Ok(outbound_queue) => {
                        // Process events through ExecutionManager (proper architectural flow)
                        // SimulationClock coordinates with ExecutionManager to process events
                        if let Err(e) =
                            self.execution_manager.process_events(symbol_id, &outbound_queue)
                        {
                            tracing::warn!(
                                "Failed to process events for symbol {}: {:?}",
                                symbol_id,
                                e
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to get OutboundQueue for symbol {}: {:?}",
                            symbol_id,
                            e
                        );
                    }
                }

                // Record metrics
                let processing_time = start_time.elapsed();
                self.metrics_collector.record_symbol_processing(symbol_id, processing_time);

                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to process tick for symbol {}: {:?}", symbol_id, e);
                Err(symbol_id)
            }
        }
    }

    /// Get all active symbols in deterministic order
    fn get_active_symbols(&self) -> Vec<SymbolId> {
        let symbols = self.active_symbols.read().unwrap();

        match &self.config.symbol_ordering {
            SymbolOrdering::BySymbolId => symbols.keys().copied().collect(),
            SymbolOrdering::ByActivationTime => {
                // For now, use symbol ID ordering
                // In the future, we can track activation time
                symbols.keys().copied().collect()
            }
            SymbolOrdering::Custom(order) => order
                .iter()
                .filter(|&&symbol_id| symbols.contains_key(&symbol_id))
                .copied()
                .collect(),
        }
    }

    /// Handle symbol processing failure
    fn handle_symbol_failure(&self, symbol_id: SymbolId) -> Result<(), ClockError> {
        match &self.config.error_recovery {
            ErrorRecovery::Continue => {
                tracing::warn!("Symbol {} failed, marking for eviction", symbol_id);
                self.mark_symbol_for_eviction(symbol_id);
                self.metrics_collector.record_symbol_failure(symbol_id);
                Ok(())
            }
            ErrorRecovery::Halt => {
                tracing::error!("Symbol {} failed, halting system", symbol_id);
                self.stop();
                Err(ClockError::SystemHalted)
            }
            ErrorRecovery::Retry(max_retries) => {
                let mut retry_counts = self.symbol_retry_counts.write().unwrap();
                let current_retries = retry_counts.get(&symbol_id).copied().unwrap_or(0);

                if current_retries < (*max_retries) as u32 {
                    tracing::warn!(
                        "Symbol {} failed, retrying ({}/{})",
                        symbol_id,
                        current_retries + 1,
                        max_retries
                    );
                    retry_counts.insert(symbol_id, current_retries + 1);
                    Ok(())
                } else {
                    tracing::warn!(
                        "Symbol {} failed after {} retries, evicting",
                        symbol_id,
                        max_retries
                    );
                    self.mark_symbol_for_eviction(symbol_id);
                    self.metrics_collector.record_symbol_failure(symbol_id);
                    Ok(())
                }
            }
        }
    }

    /// Mark symbol for eviction at next tick boundary
    fn mark_symbol_for_eviction(&self, symbol_id: SymbolId) {
        let eviction_queue = self.eviction_queue.clone();
        tokio::spawn(async move {
            let mut queue = eviction_queue.lock().await;
            queue.push(symbol_id);
        });
    }

    /// Process evictions at tick boundary
    async fn process_evictions(&self) -> Result<(), ClockError> {
        let mut eviction_queue = self.eviction_queue.lock().await;
        let to_evict = eviction_queue.drain(..).collect::<Vec<_>>();
        drop(eviction_queue);

        for symbol_id in to_evict {
            self.unregister_symbol(symbol_id)?;

            // Notify SymbolCoordinator
            self.symbol_coordinator.release_if_idle(symbol_id);
        }

        Ok(())
    }

    /// Wait for next tick (precise timing)
    fn wait_for_next_tick(&self, tick_start: Instant) -> Result<(), ClockError> {
        let elapsed = tick_start.elapsed();
        let target_duration = self.config.tick_cadence();

        if elapsed < target_duration {
            let remaining = target_duration - elapsed;
            thread::sleep(remaining);
        } else {
            // We're behind schedule
            tracing::warn!("Tick processing took {:?}, target was {:?}", elapsed, target_duration);
        }

        Ok(())
    }

    /// Emit metrics to AnalyticsEngine
    fn emit_metrics(&self, tick: TickId, tick_duration: Duration) -> Result<(), ClockError> {
        let metrics = self.metrics_collector.get_metrics();

        // For now, just log the metrics
        // In the future, we'll send to AnalyticsEngine
        tracing::debug!(
            tick = tick,
            duration_ns = tick_duration.as_nanos(),
            active_symbols = metrics.active_symbols,
            tick_rate_hz = metrics.tick_rate_hz,
            "Tick metrics"
        );

        Ok(())
    }

    /// Perform health check
    fn perform_health_check(&self) -> Result<(), ClockError> {
        let metrics = self.metrics_collector.get_metrics();

        // Check tick processing latency
        if metrics.avg_tick_duration_ns > self.config.max_tick_duration().as_nanos() as u64 {
            tracing::warn!("High tick processing latency: {}ns", metrics.avg_tick_duration_ns);
        }

        // Check symbol failure rate
        if metrics.total_symbol_failures > 0 {
            let failure_rate =
                metrics.total_symbol_failures as f64 / metrics.total_ticks_processed as f64;
            if failure_rate > 0.01 {
                tracing::warn!("High symbol failure rate: {:.2}%", failure_rate * 100.0);
            }
        }

        Ok(())
    }

    /// Create a snapshot of the current system state
    async fn create_snapshot(&self, tick: TickId) -> Result<(), ClockError> {
        use persistence::snapshot::{OrderBookState, SystemConfig, SystemState, SystemStats};
        use std::collections::HashMap;

        // Collect system state
        let active_symbols = self.symbol_coordinator.get_active_symbol_ids();

        // For now, create empty order books and accounts
        // TODO: In the future, we'll collect actual order book state from Whistle engines
        let mut order_books = HashMap::new();
        for &symbol_id in &active_symbols {
            order_books.insert(
                symbol_id,
                OrderBookState {
                    symbol_id,
                    buy_orders: HashMap::new(),
                    sell_orders: HashMap::new(),
                    last_trade_price: None,
                    last_trade_quantity: None,
                    last_trade_timestamp: None,
                },
            );
        }

        let accounts = HashMap::new(); // TODO: Collect from AccountService

        let system_config = SystemConfig {
            max_symbols: self.config.max_concurrent_symbols as u32,
            max_accounts: 1000, // TODO: Get from config
            tick_duration_ns: self.config.tick_cadence().as_nanos() as u64,
        };

        let _metrics = self.metrics_collector.get_metrics();
        let system_stats = SystemStats {
            total_orders: 0, // TODO: Collect from ExecutionManager
            total_trades: 0, // TODO: Collect from ExecutionManager
            total_volume: 0, // TODO: Collect from ExecutionManager
            current_tick: tick,
            uptime_seconds: 0, // TODO: Calculate from start time
        };

        let state = SystemState {
            order_books,
            accounts,
            active_symbols,
            config: system_config,
            stats: system_stats,
        };

        // Create snapshot via persistence backend
        let persistence = self.persistence.clone();
        let snapshot_id = tokio::task::spawn_blocking(move || {
            tokio::runtime::Handle::current()
                .block_on(async { persistence.create_snapshot(state, tick).await })
        })
        .await
        .map_err(|e| ClockError::Internal(format!("Failed to spawn snapshot task: {e}")))?
        .map_err(|e| ClockError::Internal(format!("Failed to create snapshot: {e}")))?;

        tracing::debug!("Created snapshot {} at tick {}", snapshot_id, tick);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_config_default() {
        let config = ClockConfig::default();
        assert_eq!(config.tick_cadence_ms, 1);
        assert_eq!(config.max_concurrent_symbols, 100);
    }

    #[test]
    fn test_clock_config_duration_conversion() {
        let config = ClockConfig::default();
        assert_eq!(config.tick_cadence(), Duration::from_millis(1));
        assert_eq!(config.metrics_interval(), Duration::from_millis(1000));
    }
}
