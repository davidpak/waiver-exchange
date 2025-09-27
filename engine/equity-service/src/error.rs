//! Error types for Equity Valuation Service

use thiserror::Error;

/// Result type for Equity Valuation Service operations
pub type Result<T> = std::result::Result<T, EquityServiceError>;

/// Errors that can occur in the Equity Valuation Service
#[derive(Error, Debug)]
pub enum EquityServiceError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Account not found: {0}")]
    AccountNotFound(i64),

    #[error("Symbol not found: {0}")]
    SymbolNotFound(u32),

    #[error("Position not found for account {0} and symbol {1}")]
    PositionNotFound(i64, u32),

    #[error("Invalid trade data: {0}")]
    InvalidTradeData(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("WebSocket error: {0}")]
    WebSocketError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Performance error: {0}")]
    PerformanceError(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

impl From<String> for EquityServiceError {
    fn from(err: String) -> Self {
        EquityServiceError::Internal(err)
    }
}

impl From<&str> for EquityServiceError {
    fn from(err: &str) -> Self {
        EquityServiceError::Internal(err.to_string())
    }
}
