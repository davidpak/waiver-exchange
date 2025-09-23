//! OrderGateway - WebSocket API for order submission and market data
//!
//! This module provides the external API interface for the Waiver Exchange,
//! handling WebSocket connections, authentication, rate limiting, and
//! real-time market data broadcasting.

pub mod auth;
pub mod cache;
pub mod config;
pub mod error;
pub mod gateway;
pub mod market_data_broadcaster;
pub mod messages;
pub mod oauth;
pub mod rate_limiter;
pub mod rest_api;
pub mod websocket_handler;

pub use config::GatewayConfig;
pub use error::GatewayError;
pub use gateway::OrderGateway;

/// Version of the OrderGateway API
pub const VERSION: &str = "0.1.0";

/// Default WebSocket port
pub const DEFAULT_PORT: u16 = 8080;

/// Default maximum connections
pub const DEFAULT_MAX_CONNECTIONS: usize = 10000;

/// Default heartbeat interval in seconds
pub const DEFAULT_HEARTBEAT_INTERVAL: u64 = 30;
