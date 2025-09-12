//! Error types for the persistence layer

use thiserror::Error;

/// Result type alias for persistence operations
pub type Result<T> = std::result::Result<T, PersistenceError>;

/// Errors that can occur in the persistence layer
#[derive(Error, Debug)]
pub enum PersistenceError {
    /// I/O errors (file operations, network, etc.)
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Bincode serialization errors
    #[error("Bincode serialization error: {0}")]
    Bincode(#[from] bincode::Error),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Invalid data format or corruption
    #[error("Data corruption: {0}")]
    Corruption(String),

    /// Resource not found
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Resource already exists
    #[error("Resource already exists: {0}")]
    AlreadyExists(String),

    /// Invalid operation for current state
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Timeout errors
    #[error("Operation timeout: {0}")]
    Timeout(String),

    /// Generic error with context
    #[error("Persistence error: {0}")]
    Generic(String),
}

impl PersistenceError {
    /// Create a new configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create a new corruption error
    pub fn corruption(msg: impl Into<String>) -> Self {
        Self::Corruption(msg.into())
    }

    /// Create a new not found error
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    /// Create a new already exists error
    pub fn already_exists(msg: impl Into<String>) -> Self {
        Self::AlreadyExists(msg.into())
    }

    /// Create a new invalid operation error
    pub fn invalid_operation(msg: impl Into<String>) -> Self {
        Self::InvalidOperation(msg.into())
    }

    /// Create a new timeout error
    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }

    /// Create a new generic error
    pub fn generic(msg: impl Into<String>) -> Self {
        Self::Generic(msg.into())
    }
}
