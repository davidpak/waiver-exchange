use crate::SymbolCoordinatorApi;
use crate::placement::{EngineThreadPool, PlacementPolicy, RoundRobinPolicy};
use crate::queue::QueueAllocator;
use crate::registry::SymbolRegistry;
use crate::types::{CoordError, CoordinatorConfig, ReadyAtTick, SymbolId};
use execution_manager::ExecutionManager;
use std::sync::{Arc, Mutex};
use whistle::TickId;
use whistle::{
    BandMode, Bands, EngineCfg, ExecIdMode, PriceDomain, ReferencePriceSource, SelfMatchPolicy,
    Whistle,
};

/// Type alias for order book state to reduce complexity
type OrderBookState = (Vec<(u32, u64)>, Vec<(u32, u64)>);

/// Type alias for trade info to reduce complexity
type TradeInfo = (Option<u64>, Option<u64>, Option<chrono::DateTime<chrono::Utc>>);

/// Main SymbolCoordinator implementation
/// Uses interior mutability to allow mutation through immutable references
pub struct SymbolCoordinator {
    inner: Arc<Mutex<SymbolCoordinatorInner>>,
}

/// Internal state that can be mutated
struct SymbolCoordinatorInner {
    config: CoordinatorConfig,
    registry: SymbolRegistry,
    thread_pool: EngineThreadPool,
    placement_policy: Box<dyn PlacementPolicy>,
    queue_allocator: QueueAllocator,
    current_tick: TickId,
    execution_manager: Arc<ExecutionManager>,
}

// SAFETY: SymbolCoordinatorInner is safe to send and sync because:
// - All fields are Send + Sync
// - PlacementPolicy trait requires Debug which implies Send + Sync
unsafe impl Send for SymbolCoordinatorInner {}
unsafe impl Sync for SymbolCoordinatorInner {}

impl SymbolCoordinator {
    pub fn new(config: CoordinatorConfig, execution_manager: Arc<ExecutionManager>) -> Self {
        let num_threads = config.num_threads;
        let spsc_depth = config.spsc_depth;
        let placement_policy = Box::new(RoundRobinPolicy::new(num_threads));
        let queue_allocator = QueueAllocator::new(spsc_depth);

        let inner = SymbolCoordinatorInner {
            config,
            registry: SymbolRegistry::new(),
            thread_pool: EngineThreadPool::new(num_threads),
            placement_policy,
            queue_allocator,
            current_tick: 0,
            execution_manager,
        };

        Self { inner: Arc::new(Mutex::new(inner)) }
    }

    /// Create default EngineCfg for a symbol
    fn create_default_engine_config(&self, spsc_depth: usize, symbol_id: SymbolId) -> EngineCfg {
        EngineCfg {
            symbol: symbol_id,
            price_domain: PriceDomain { floor: 100, ceil: 100000, tick: 1 }, // $1.00 to $1000.00, $0.01 tick
            bands: Bands { mode: BandMode::Percent(10) },                   // 10% bands
            batch_max: spsc_depth as u32,
            arena_capacity: 1024, // Max 1024 open orders (power of 2)
            elastic_arena: false,
            exec_shift_bits: 16,
            exec_id_mode: ExecIdMode::Sharded,
            self_match_policy: SelfMatchPolicy::Skip,
            allow_market_cold_start: false,
            reference_price_source: ReferencePriceSource::MidpointOnWarm,
        }
    }

    /// Update the current tick (called by SimulationClock)
    pub fn update_tick(&mut self, new_tick: TickId) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.current_tick = new_tick;
            inner.registry.update_tick(new_tick);
        }
    }

    /// Get the current tick
    pub fn current_tick(&self) -> TickId {
        if let Ok(inner) = self.inner.lock() { inner.current_tick } else { 0 }
    }

    /// Get the current tick (returns Result for external API)
    pub fn get_current_tick(&self) -> Result<TickId, CoordError> {
        if let Ok(inner) = self.inner.lock() {
            Ok(inner.current_tick)
        } else {
            Err(CoordError::Unknown)
        }
    }

    /// Get configuration
    pub fn config(&self) -> CoordinatorConfig {
        if let Ok(inner) = self.inner.lock() {
            inner.config.clone()
        } else {
            CoordinatorConfig::default()
        }
    }

    // TODO: Add test methods back once Clone traits are properly implemented
    // For now, focus on core functionality

    /// Get the SPSC queue writer for a symbol (for OrderRouter)
    pub fn get_spsc_writer(&self, symbol_id: SymbolId) -> Option<crate::types::OrderQueueWriter> {
        if let Ok(inner) = self.inner.lock() {
            inner.registry.get_entry(symbol_id).map(|entry| entry.whistle_handle.order_tx.clone())
        } else {
            None
        }
    }

    /// Get active symbols count
    pub fn active_symbols_count(&self) -> usize {
        if let Ok(inner) = self.inner.lock() {
            inner.registry.get_active_symbols().len()
        } else {
            0
        }
    }

    /// Get total registered symbols count
    pub fn total_symbols_count(&self) -> usize {
        if let Ok(inner) = self.inner.lock() { inner.registry.len() } else { 0 }
    }

    /// Get all active symbol IDs
    pub fn get_active_symbols(&self) -> Vec<u32> {
        if let Ok(inner) = self.inner.lock() {
            inner.registry.get_active_symbols()
        } else {
            Vec::new()
        }
    }

    /// Get order book state for a specific symbol
    pub fn get_order_book_state(&self, symbol_id: SymbolId) -> Option<OrderBookState> {
        if let Ok(inner) = self.inner.lock() {
            if let Some(entry) = inner.registry.get_entry(symbol_id) {
                if entry.state == crate::types::SymbolState::Active {
                    let buy_orders =
                        entry.whistle_handle.engine.get_order_book_levels(whistle::Side::Buy);
                    let sell_orders =
                        entry.whistle_handle.engine.get_order_book_levels(whistle::Side::Sell);
                    return Some((buy_orders, sell_orders));
                }
            }
        }
        None
    }

    /// Restore order book state for a specific symbol
    pub fn restore_order_book_state(
        &self,
        symbol_id: SymbolId,
        buy_orders: &std::collections::HashMap<u64, u64>,
        sell_orders: &std::collections::HashMap<u64, u64>,
        last_trade_price: Option<u64>,
        last_trade_quantity: Option<u64>,
        last_trade_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(entry) = inner.registry.get_entry_mut(symbol_id) {
                if entry.state == crate::types::SymbolState::Active {
                    entry.whistle_handle.engine.restore_order_book_state(
                        buy_orders,
                        sell_orders,
                        last_trade_price,
                        last_trade_quantity,
                        last_trade_timestamp,
                    );
                    tracing::info!("Restored order book state for symbol {}", symbol_id);
                    return Ok(());
                }
            }
        }
        Err(format!("Failed to restore order book state for symbol {symbol_id}").into())
    }

    /// Get the latest trade information for a symbol
    pub fn get_last_trade_info(
        &self,
        symbol_id: SymbolId,
    ) -> Result<TradeInfo, Box<dyn std::error::Error>> {
        if let Ok(inner) = self.inner.lock() {
            if let Some(entry) = inner.registry.get_entry(symbol_id) {
                if entry.state == crate::types::SymbolState::Active {
                    return Ok(entry.whistle_handle.engine.get_last_trade_info());
                }
            }
        }
        Err(format!("Failed to get trade info for symbol {symbol_id}").into())
    }

    /// Process a tick for a specific symbol
    /// This is the main method for SessionEngine to use
    pub fn process_symbol_tick(
        &mut self,
        symbol_id: u32,
        tick: TickId,
    ) -> Option<Vec<whistle::EngineEvent>> {
        if let Ok(mut inner) = self.inner.lock() {
            // Update current tick
            inner.current_tick = tick;
            inner.registry.update_tick(tick);

            // Process the specific symbol
            if let Some(entry) = inner.registry.get_entry_mut(symbol_id) {
                if entry.state == crate::types::SymbolState::Active {
                    // Call tick() on the Whistle engine
                    let events = entry.whistle_handle.engine.tick(tick);
                    return Some(events);
                }
            }
        }

        None
    }
}

impl SymbolCoordinatorApi for SymbolCoordinator {
    fn ensure_active(&self, symbol_id: u32) -> Result<ReadyAtTick, CoordError> {
        if let Ok(mut inner) = self.inner.lock() {
            // Check if symbol is already active
            if inner.registry.is_symbol_active(symbol_id) {
                return Ok(ReadyAtTick {
                    next_tick: inner.current_tick,
                    queue_writer: inner
                        .registry
                        .get_entry(symbol_id)
                        .ok_or(CoordError::Unknown)?
                        .whistle_handle
                        .order_tx
                        .clone(),
                });
            }

            // Symbol is not active, activate it now
            let thread_id = inner.placement_policy.assign_thread(symbol_id);

            // Create Whistle engine configuration
            let engine_cfg = self.create_default_engine_config(inner.config.spsc_depth, symbol_id);

            // Create Whistle instance (this validates the config)
            let _whistle = Whistle::new(engine_cfg);

            // Create SPSC queue for order routing
            let spsc_queue = inner.queue_allocator.create_queue();

            // Register symbol in registry
            inner
                .registry
                .register_symbol(symbol_id, thread_id, spsc_queue)
                .map_err(|_| CoordError::Unknown)?;

            // Register symbol with ExecutionManager (per documentation: SymbolCoordinator registers symbols during engine boot)
            inner.execution_manager.register_symbol(symbol_id);
            tracing::info!(
                "Registered symbol {} with ExecutionManager during engine boot",
                symbol_id
            );

            // Assign to thread pool
            inner.thread_pool.assign_symbol(thread_id).map_err(|_| CoordError::Unknown)?;

            // Activate the symbol
            inner.registry.activate_symbol(symbol_id).map_err(|_| CoordError::Unknown)?;

            // Get the queue writer for OrderRouter
            let queue_writer = inner
                .registry
                .get_entry(symbol_id)
                .ok_or(CoordError::Unknown)?
                .whistle_handle
                .order_tx
                .clone();

            Ok(ReadyAtTick { next_tick: inner.current_tick, queue_writer })
        } else {
            Err(CoordError::Unknown)
        }
    }

    fn release_if_idle(&self, _symbol_id: u32) {
        // For now, do nothing - in a real implementation, this would:
        // 1. Check if symbol is idle (no recent activity)
        // 2. If idle, mark for eviction
        // 3. Schedule cleanup at next tick boundary
    }
}

impl SymbolCoordinator {
    /// Get a list of all active symbol IDs (for SimulationClock integration)
    pub fn get_active_symbol_ids(&self) -> Vec<u32> {
        if let Ok(inner) = self.inner.lock() {
            inner.registry.get_active_symbol_ids()
        } else {
            Vec::new()
        }
    }

    /// Process a symbol tick (for SimulationClock integration)
    /// This method can be called concurrently and handles the mutable access internally
    pub fn process_symbol_tick_concurrent(
        &self,
        symbol_id: u32,
        tick: TickId,
    ) -> Result<Vec<whistle::EngineEvent>, CoordError> {
        if let Ok(mut inner) = self.inner.lock() {
            // Update current tick
            inner.current_tick = tick;
            inner.registry.update_tick(tick);

            // Process the specific symbol
            if let Some(entry) = inner.registry.get_entry_mut(symbol_id) {
                if entry.state == crate::types::SymbolState::Active {
                    // Debug: Check inbound queue length before processing
                    let inbound_queue_len = entry.whistle_handle.engine.queue_stats().0;
                    if inbound_queue_len > 0 {
                        tracing::info!(
                            "Symbol {} has {} messages in inbound queue before processing tick {}",
                            symbol_id,
                            inbound_queue_len,
                            tick
                        );
                    }

                    // Call tick_with_queue_emission() on the Whistle engine
                    // This emits events directly to the OutboundQueue instead of returning them
                    entry.whistle_handle.engine.tick_with_queue_emission(tick);

                    // Check if there are events in the outbound queue
                    let queue_len = entry.whistle_handle.outbound_queue.len();
                    if queue_len > 0 {
                        tracing::info!(
                            "Symbol {} has {} events in OutboundQueue at tick {}",
                            symbol_id,
                            queue_len,
                            tick
                        );
                    } else if tick % 1000 == 0 {
                        tracing::debug!(
                            "Symbol {} has 0 events in OutboundQueue at tick {}",
                            symbol_id,
                            tick
                        );
                    }

                    // Return empty events since they're now in the OutboundQueue
                    return Ok(Vec::new());
                }
            }

            Err(CoordError::Unknown)
        } else {
            Err(CoordError::Unknown)
        }
    }

    /// Update the current tick (called by SimulationClock)
    pub fn update_current_tick(&self, tick: TickId) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.current_tick = tick;
            inner.registry.update_tick(tick);
        }
    }

    /// Get the OutboundQueue for a symbol (for ExecutionManager integration)
    pub fn get_outbound_queue(
        &self,
        symbol_id: SymbolId,
    ) -> Result<std::sync::Arc<whistle::OutboundQueue>, CoordError> {
        if let Ok(inner) = self.inner.lock() {
            if let Some(entry) = inner.registry.get_entry(symbol_id) {
                Ok(entry.whistle_handle.outbound_queue.clone())
            } else {
                Err(CoordError::Unknown)
            }
        } else {
            Err(CoordError::Unknown)
        }
    }
}
