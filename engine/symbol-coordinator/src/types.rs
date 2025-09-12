use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use whistle::TickId;

/// Symbol identifier (matches order-router)
pub type SymbolId = u32;

/// Thread identifier for engine placement
pub type ThreadId = u32;

/// Engine state for lifecycle management
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SymbolState {
    Unregistered,
    Registered,
    Active,
    Evicting,
    Evicted,
}

/// Response from SymbolCoordinator when activating a symbol
#[derive(Debug, Clone)]
pub struct ReadyAtTick {
    pub next_tick: TickId,
    pub queue_writer: OrderQueueWriter,
}

/// Error types from SymbolCoordinator
#[derive(Debug, Clone, PartialEq)]
pub enum CoordError {
    Capacity,
    Faulted,
    Unknown,
}

/// Re-export for compatibility with order-router
pub type OrderRouterCoordError = CoordError;

/// Engine metadata for tracking
#[derive(Debug, Clone)]
pub struct EngineMetadata {
    pub symbol_id: SymbolId,
    pub thread_id: ThreadId,
    pub state: SymbolState,
    pub created_at: TickId,
}

/// Handle for Whistle engine operations
/// Stores the actual Whistle instance for SimulationClock to access
/// NO LOCKS ON HOT PATH - SimulationClock needs direct access to call tick()
pub struct WhistleHandle {
    pub order_tx: OrderQueueWriter,
    pub metadata: EngineMetadata,
    pub tick_flag: AtomicBool,
    pub engine: whistle::Whistle, // Direct access for SimulationClock - no locks
    pub outbound_queue: Arc<whistle::OutboundQueue>, // For ExecutionManager integration
}

// TODO: Implement proper Clone for WhistleHandle
// This requires Whistle to implement Clone, which may not be feasible
// For now, we'll use a different approach in SimulationClock

/// Write handle for OrderRouter to enqueue orders
/// Lock-free SPSC queue access - NO LOCKS ON HOT PATH
///
/// This provides direct access to the InboundQueue without going through Whistle
/// to avoid locks on the hot path. The queue itself is lock-free internally.
#[derive(Debug, Clone)]
pub struct OrderQueueWriter {
    pub queue: Arc<whistle::InboundQueue>,
}

impl OrderQueueWriter {
    pub fn new(queue: whistle::InboundQueue) -> Self {
        Self { queue: Arc::new(queue) }
    }

    pub fn try_enqueue(&mut self, msg: whistle::InboundMsg) -> Result<(), whistle::RejectReason> {
        // Use the lock-free interface - NO LOCKS ON HOT PATH
        self.queue.try_enqueue_lockfree(msg)
    }
}

/// Configuration for symbol coordinator
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    pub num_threads: u32,
    pub spsc_depth: usize,
    pub max_symbols_per_thread: u32,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self { num_threads: 4, spsc_depth: 2048, max_symbols_per_thread: 64 }
    }
}
