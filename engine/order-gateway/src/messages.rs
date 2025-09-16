//! Message types for the OrderGateway WebSocket API

use serde::{Deserialize, Serialize};

/// Base message structure for all WebSocket messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message ID for request/response correlation
    pub id: Option<String>,

    /// Message method (for requests) or stream type (for responses)
    pub method: Option<String>,
    pub stream: Option<String>,

    /// Message parameters (for requests) or data (for responses)
    pub params: Option<serde_json::Value>,
    pub data: Option<serde_json::Value>,

    /// Result (for successful responses)
    pub result: Option<serde_json::Value>,

    /// Error (for error responses)
    pub error: Option<ErrorMessage>,
}

/// Error message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    /// Error code
    pub code: u32,

    /// Error message
    pub message: String,

    /// Additional error details
    pub details: Option<serde_json::Value>,
}

/// Order placement request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderPlaceRequest {
    /// Symbol to trade
    pub symbol: String,

    /// Order side (BUY/SELL)
    pub side: String,

    /// Order type (LIMIT/MARKET/IOC/FOK)
    #[serde(rename = "type")]
    pub r#type: String,

    /// Order price (for limit orders)
    pub price: u32,

    /// Order quantity
    pub quantity: u64,

    /// Account ID
    pub account_id: String,

    /// Client order ID (optional)
    pub client_order_id: Option<String>,
}

/// Order placement response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderPlaceResponse {
    /// Order ID assigned by the system
    pub order_id: String,

    /// Order status
    pub status: String,

    /// Timestamp
    pub timestamp: u64,

    /// Client order ID (if provided)
    pub client_order_id: Option<String>,
}

/// Authentication request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    /// API key
    pub api_key: String,

    /// API secret
    pub api_secret: String,
}

/// Authentication response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    /// Whether authentication was successful
    pub authenticated: bool,

    /// User ID
    pub user_id: Option<String>,

    /// User permissions
    pub permissions: Vec<String>,

    /// Rate limits
    pub rate_limits: RateLimits,
}

/// Rate limits information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimits {
    /// Orders per second
    pub orders_per_second: u32,

    /// Market data updates per second
    pub market_data_per_second: u32,

    /// Burst limit
    pub burst_limit: u32,
}

/// Market data update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataUpdate {
    /// Symbol
    pub symbol: String,

    /// Bid levels (price, quantity)
    pub bids: Vec<[u32; 2]>,

    /// Ask levels (price, quantity)
    pub asks: Vec<[u32; 2]>,

    /// Last trade information
    pub last_trade: Option<LastTrade>,

    /// Current tick
    pub tick: u64,

    /// Timestamp
    pub timestamp: u64,
}

/// Last trade information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastTrade {
    /// Trade price
    pub price: u32,

    /// Trade quantity
    pub quantity: u64,

    /// Trade timestamp
    pub timestamp: u64,
}

/// Order status update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderStatusUpdate {
    /// Order ID
    pub order_id: String,

    /// Order status
    pub status: String,

    /// Filled quantity
    pub filled_quantity: u64,

    /// Average fill price
    pub average_price: Option<u32>,

    /// Timestamp
    pub timestamp: u64,

    /// Client order ID (if provided)
    pub client_order_id: Option<String>,
}

/// User session information
#[derive(Debug, Clone)]
pub struct UserSession {
    /// User ID
    pub user_id: String,

    /// User permissions
    pub permissions: Vec<String>,

    /// Rate limits
    pub rate_limits: RateLimits,

    /// Session start time
    pub start_time: std::time::Instant,

    /// Last activity time
    pub last_activity: std::time::Instant,
}

impl UserSession {
    /// Create a new user session
    pub fn new(user_id: String, permissions: Vec<String>, rate_limits: RateLimits) -> Self {
        let now = std::time::Instant::now();
        Self { user_id, permissions, rate_limits, start_time: now, last_activity: now }
    }

    /// Update last activity time
    pub fn update_activity(&mut self) {
        self.last_activity = std::time::Instant::now();
    }

    /// Check if user has permission
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(&permission.to_string())
    }
}
