use crate::placement::{EngineThreadPool, PlacementPolicy, RoundRobinPolicy};
use crate::queue::QueueAllocator;
use crate::registry::SymbolRegistry;
use crate::types::{CoordinatorConfig, SymbolId};
use order_router::{CoordError as OrderRouterCoordError, ReadyAtTick, SymbolCoordinatorApi};
use std::sync::{Arc, Mutex};
use whistle::TickId;
use whistle::{
    BandMode, Bands, EngineCfg, ExecIdMode, PriceDomain, ReferencePriceSource, SelfMatchPolicy,
    Whistle,
};

/// Main SymbolCoordinator implementation
/// Uses interior mutability to allow mutation through immutable references
#[derive(Debug)]
pub struct SymbolCoordinator {
    inner: Arc<Mutex<SymbolCoordinatorInner>>,
}

/// Internal state that can be mutated
#[derive(Debug)]
struct SymbolCoordinatorInner {
    config: CoordinatorConfig,
    registry: SymbolRegistry,
    thread_pool: EngineThreadPool,
    placement_policy: Box<dyn PlacementPolicy>,
    queue_allocator: QueueAllocator,
    current_tick: TickId,
}

// SAFETY: SymbolCoordinatorInner is safe to send and sync because:
// - All fields are Send + Sync
// - PlacementPolicy trait requires Debug which implies Send + Sync
unsafe impl Send for SymbolCoordinatorInner {}
unsafe impl Sync for SymbolCoordinatorInner {}

impl SymbolCoordinator {
    pub fn new(config: CoordinatorConfig) -> Self {
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
        };

        Self { inner: Arc::new(Mutex::new(inner)) }
    }

    /// Create default EngineCfg for a symbol
    fn create_default_engine_config(&self, spsc_depth: usize, symbol_id: SymbolId) -> EngineCfg {
        EngineCfg {
            symbol: symbol_id,
            price_domain: PriceDomain { floor: 100, ceil: 10000, tick: 1 }, // $1.00 to $100.00, $0.01 tick
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
        if let Ok(inner) = self.inner.lock() {
            inner.current_tick
        } else {
            0
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
        if let Ok(inner) = self.inner.lock() {
            inner.registry.len()
        } else {
            0
        }
    }
}

impl SymbolCoordinatorApi for SymbolCoordinator {
    fn ensure_active(&self, symbol_id: u32) -> Result<ReadyAtTick, OrderRouterCoordError> {
        if let Ok(mut inner) = self.inner.lock() {
            // Check if symbol is already active
            if inner.registry.is_symbol_active(symbol_id) {
                return Ok(ReadyAtTick { next_tick: inner.current_tick });
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
                .map_err(|_| OrderRouterCoordError::Unknown)?;

            // Assign to thread pool
            inner
                .thread_pool
                .assign_symbol(thread_id)
                .map_err(|_| OrderRouterCoordError::Unknown)?;

            // Activate the symbol
            inner
                .registry
                .activate_symbol(symbol_id)
                .map_err(|_| OrderRouterCoordError::Unknown)?;

            Ok(ReadyAtTick { next_tick: inner.current_tick })
        } else {
            Err(OrderRouterCoordError::Unknown)
        }
    }

    fn release_if_idle(&self, _symbol_id: u32) {
        // For now, do nothing - in a real implementation, this would:
        // 1. Check if symbol is idle (no recent activity)
        // 2. If idle, mark for eviction
        // 3. Schedule cleanup at next tick boundary
    }
}
