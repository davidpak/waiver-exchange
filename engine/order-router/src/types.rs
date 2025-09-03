use std::sync::Arc;
use whistle::TickId;

/// Message with symbol ID for routing
#[derive(Debug, Clone)]
pub struct InboundMsgWithSymbol {
    pub symbol_id: u32,
    pub msg: whistle::InboundMsg,
}

/// Response from SymbolCoordinator when activating a symbol
#[derive(Debug, Clone)]
pub struct ReadyAtTick {
    pub next_tick: TickId,
    pub queue_writer: OrderQueueWriter,
}

/// Write handle for OrderRouter to enqueue orders
/// This provides access to the InboundQueue from SymbolCoordinator
#[derive(Debug, Clone)]
pub struct OrderQueueWriter {
    pub queue: Arc<whistle::InboundQueue>,
}

/// Error types from SymbolCoordinator
#[derive(Debug, Clone, PartialEq)]
pub enum CoordError {
    Capacity,
    Faulted,
    Unknown,
}

/// Trait that SymbolCoordinator must implement for OrderRouter integration
pub trait SymbolCoordinatorApi: Send {
    fn ensure_active(&self, symbol_id: u32) -> Result<ReadyAtTick, CoordError>;
    fn release_if_idle(&self, symbol_id: u32);
}

/// Symbol-to-shard mapping result
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymbolShard {
    pub symbol_id: u32,
    pub shard_id: u32,
}

/// Router performance metrics
#[derive(Debug, Clone, Default)]
pub struct RouterMetrics {
    pub enqueued: u64,
    pub rejected_backpressure: u64,
    pub rejected_inactive: u64,
    pub activation_requests: u64,
    pub active_symbols: u32,
}
