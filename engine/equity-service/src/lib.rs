//! Equity Valuation Service (EVS) - real-time equity calculation and tracking
//!
//! This service subscribes to ExecutionManager events and calculates real-time equity
//! for all accounts. It maintains in-memory caches for performance and broadcasts
//! equity updates via WebSocket to the frontend.

mod config;
mod error;
mod types;
mod service;

pub use config::EquityServiceConfig;
pub use error::{EquityServiceError, Result};
pub use types::{
    AccountEquityData, EquitySnapshot, Position, EquityUpdate, EquityBroadcaster,
};
pub use service::EquityValuationService;

/// Re-export commonly used types
pub use execution_manager::{DispatchEvent, TradeEvent, BookDelta, TickBoundaryEvent};
pub use account_service::trade::TradeDetails;
pub use account_service::position::TradeSide;
