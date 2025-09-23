// SymbolCoordinator - per-symbol engine lifecycle management
#![allow(dead_code)]

mod coordinator;
mod placement;
mod queue;
mod registry;
mod types;

pub use coordinator::SymbolCoordinator;
pub use types::{
    CoordError, CoordinatorConfig, OrderQueueWriter, ReadyAtTick, SymbolId, WhistleHandle,
};

// Define the trait locally for OrderRouter compatibility
pub trait SymbolCoordinatorApi: Send + Sync {
    fn ensure_active(&self, symbol_id: u32) -> Result<ReadyAtTick, CoordError>;
    fn release_if_idle(&self, symbol_id: u32);
}
