//! Error types for the OrderGateway

use thiserror::Error;

/// Errors that can occur in the OrderGateway
#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("Invalid order parameters: {0}")]
    InvalidOrder(String),

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("System error: {0}")]
    System(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Order router error: {0}")]
    OrderRouter(String),

    #[error("Execution manager error: {0}")]
    ExecutionManager(String),
}

/// Auth errors for warp rejections
#[derive(Debug)]
pub enum AuthError {
    MissingCode,
    OAuthFailed,
}

impl warp::reject::Reject for AuthError {}

impl From<String> for GatewayError {
    fn from(err: String) -> Self {
        GatewayError::System(err)
    }
}

/// Result type for OrderGateway operations
pub type GatewayResult<T> = Result<T, GatewayError>;
