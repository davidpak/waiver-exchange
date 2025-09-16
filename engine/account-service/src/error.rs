//! Error types for AccountService

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AccountServiceError {
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: u64, available: u64 },
    
    #[error("Account not found: {account_id}")]
    AccountNotFound { account_id: i64 },
    
    #[error("Reservation not found: {reservation_id}")]
    ReservationNotFound { reservation_id: u64 },
    
    #[error("Sleeper API error: {message}")]
    SleeperApiError { message: String },
    
    #[error("Google OAuth error: {message}")]
    GoogleOAuthError { message: String },
    
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    
    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),
    
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    
    #[error("OAuth error: {0}")]
    OAuth(#[from] oauth2::RequestTokenError<oauth2::reqwest::Error<reqwest::Error>, oauth2::basic::BasicErrorResponse>),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Invalid configuration: {message}")]
    InvalidConfig { message: String },
    
    #[error("Internal error: {message}")]
    Internal { message: String },
}
