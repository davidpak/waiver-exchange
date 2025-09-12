//! Error types for SimulationClock

use symbol_coordinator::SymbolId;
use thiserror::Error;

/// Errors that can occur in the SimulationClock
#[derive(Error, Debug)]
pub enum ClockError {
    #[error("Symbol error: {0}")]
    Symbol(#[from] SymbolError),

    #[error("System error: {0}")]
    System(#[from] SystemError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Symbol {symbol_id} is not ready for registration")]
    SymbolNotReady { symbol_id: SymbolId },

    #[error("Symbol {symbol_id} is already registered")]
    SymbolAlreadyRegistered { symbol_id: SymbolId },

    #[error("Symbol {symbol_id} is not registered")]
    SymbolNotRegistered { symbol_id: SymbolId },

    #[error("Clock is not running")]
    ClockNotRunning,

    #[error("Clock is already running")]
    ClockAlreadyRunning,

    #[error("Tick processing timeout")]
    TickTimeout,

    #[error("System halted due to critical failure")]
    SystemHalted,

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Errors related to symbol processing
#[derive(Error, Debug)]
pub enum SymbolError {
    #[error("Symbol {symbol_id} engine crashed")]
    EngineCrash { symbol_id: SymbolId },

    #[error("Symbol {symbol_id} processing timeout")]
    ProcessingTimeout { symbol_id: SymbolId },

    #[error("Symbol {symbol_id} queue overflow")]
    QueueOverflow { symbol_id: SymbolId },

    #[error("Symbol {symbol_id} memory allocation failure")]
    MemoryAllocationFailure { symbol_id: SymbolId },

    #[error("Symbol {symbol_id} invalid state")]
    InvalidState { symbol_id: SymbolId },
}

/// Errors related to system components
#[derive(Error, Debug)]
pub enum SystemError {
    #[error("ExecutionManager failure: {0}")]
    ExecutionManagerFailure(String),

    #[error("Persistence failure: {0}")]
    PersistenceFailure(String),

    #[error("OrderGateway failure: {0}")]
    OrderGatewayFailure(String),

    #[error("AnalyticsEngine failure: {0}")]
    AnalyticsEngineFailure(String),

    #[error("Thread pool exhaustion")]
    ThreadPoolExhaustion,

    #[error("Memory exhaustion")]
    MemoryExhaustion,

    #[error("Disk space exhaustion")]
    DiskSpaceExhaustion,

    #[error("Network failure: {0}")]
    NetworkFailure(String),
}

impl From<SymbolId> for SymbolError {
    fn from(symbol_id: SymbolId) -> Self {
        SymbolError::EngineCrash { symbol_id }
    }
}
