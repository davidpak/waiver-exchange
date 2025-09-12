//! # SimulationClock
//!
//! The system heartbeat that drives logical time progression and coordinates all trading components.
//!
//! The SimulationClock serves as the central coordinator that autonomously manages tick processing,
//! symbol lifecycle, and system coordination, transforming the system from a collection of manual
//! components into a self-running, production-ready trading platform.

pub mod clock;
pub mod config;
pub mod error;
pub mod metrics;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_tests;

pub use clock::SimulationClock;
pub use config::ClockConfig;
pub use error::{ClockError, SymbolError, SystemError};

pub use symbol_coordinator::{SymbolId, WhistleHandle};
/// Re-export commonly used types
pub use whistle::{EngineEvent, TickId};

/// Current version of the SimulationClock
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default tick cadence (1kHz = 1ms per tick)
pub const DEFAULT_TICK_CADENCE_MS: u64 = 1;

/// Default maximum concurrent symbols
pub const DEFAULT_MAX_CONCURRENT_SYMBOLS: usize = 100;

/// Default metrics emission interval
pub const DEFAULT_METRICS_INTERVAL_MS: u64 = 1000;
