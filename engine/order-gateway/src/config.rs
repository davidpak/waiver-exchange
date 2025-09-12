//! Configuration for the OrderGateway

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Main configuration for the OrderGateway
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GatewayConfig {
    /// Server configuration
    pub server: ServerConfig,

    /// Rate limiting configuration
    pub rate_limits: RateLimitConfig,

    /// Authentication configuration
    pub auth: AuthConfig,

    /// Market data configuration
    pub market_data: MarketDataConfig,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,

    /// Port to bind to
    pub port: u16,

    /// Maximum number of concurrent connections
    pub max_connections: usize,

    /// Heartbeat interval in seconds
    pub heartbeat_interval: u64,

    /// Connection timeout in seconds
    pub connection_timeout: u64,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Orders per second per user
    pub orders_per_second: u32,

    /// Market data updates per second per user
    pub market_data_per_second: u32,

    /// Burst limit for orders
    pub burst_limit: u32,

    /// Rate limit window in seconds
    pub window_seconds: u64,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable API key authentication
    pub api_key_validation: bool,

    /// API key validation endpoint (if external)
    pub validation_endpoint: Option<String>,

    /// Default permissions for authenticated users
    pub default_permissions: Vec<String>,
}

/// Market data configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataConfig {
    /// Enable market data broadcasting
    pub enabled: bool,

    /// Maximum market data updates per second
    pub max_updates_per_second: u32,

    /// Market data compression
    pub compression: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            max_connections: 10000,
            heartbeat_interval: 30,
            connection_timeout: 60,
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            orders_per_second: 100,
            market_data_per_second: 1000,
            burst_limit: 10,
            window_seconds: 1,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            api_key_validation: true,
            validation_endpoint: None,
            default_permissions: vec!["trade".to_string(), "market_data".to_string()],
        }
    }
}

impl Default for MarketDataConfig {
    fn default() -> Self {
        Self { enabled: true, max_updates_per_second: 1000, compression: false }
    }
}

impl GatewayConfig {
    /// Get the server address
    pub fn server_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.server.host, self.server.port).parse()
    }

    /// Load configuration from file
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: GatewayConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn save_to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
