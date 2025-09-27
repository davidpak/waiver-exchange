//! Configuration for Equity Valuation Service

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the Equity Valuation Service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityServiceConfig {
    /// Database connection URL
    pub database_url: String,
    
    /// Cache configuration
    pub cache: CacheConfig,
    
    /// WebSocket configuration
    pub websocket: WebSocketConfig,
    
    /// Performance configuration
    pub performance: PerformanceConfig,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum number of accounts to cache
    pub max_accounts: usize,
    
    /// Maximum number of symbols to cache prices for
    pub max_symbols: usize,
    
    /// Cache TTL for account data
    pub account_ttl: Duration,
    
    /// Cache TTL for price data
    pub price_ttl: Duration,
}

/// WebSocket configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    /// Maximum number of WebSocket connections per account
    pub max_connections_per_account: usize,
    
    /// WebSocket message buffer size
    pub buffer_size: usize,
    
    /// WebSocket heartbeat interval
    pub heartbeat_interval: Duration,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Maximum processing time per tick
    pub max_tick_processing_time: Duration,
    
    /// Maximum number of events to process per batch
    pub max_events_per_batch: usize,
    
    /// Enable performance metrics
    pub enable_metrics: bool,
}

impl Default for EquityServiceConfig {
    fn default() -> Self {
        Self {
            database_url: "postgresql://localhost/waiver_exchange".to_string(),
            cache: CacheConfig::default(),
            websocket: WebSocketConfig::default(),
            performance: PerformanceConfig::default(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_accounts: 10000,
            max_symbols: 1000,
            account_ttl: Duration::from_secs(300), // 5 minutes
            price_ttl: Duration::from_secs(60),    // 1 minute
        }
    }
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            max_connections_per_account: 5,
            buffer_size: 1000,
            heartbeat_interval: Duration::from_secs(30),
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_tick_processing_time: Duration::from_millis(100),
            max_events_per_batch: 1000,
            enable_metrics: true,
        }
    }
}

impl EquityServiceConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/waiver_exchange".to_string());
        
        Ok(Self {
            database_url,
            cache: CacheConfig::default(),
            websocket: WebSocketConfig::default(),
            performance: PerformanceConfig::default(),
        })
    }
}
