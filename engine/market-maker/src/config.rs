use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Market Maker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMakerConfig {
    pub database: DatabaseConfig,
    pub market_maker: MarketMakerParameters,
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMakerParameters {
    /// Whether market maker is enabled
    pub enabled: bool,
    
    /// Spread around fair price in basis points (e.g., 400 = ±4%)
    pub spread_bps: u32,
    
    /// How often to check and update quotes (seconds)
    pub update_frequency_seconds: u64,
    
    /// Maximum position per player (shares)
    pub max_position_per_player: i64,
    
    /// Daily notional limit in cents
    pub daily_notional_limit_cents: i64,
    
    /// Risk check interval (seconds)
    pub risk_check_interval_seconds: u64,
    
    /// Maximum spread before posting (basis points)
    pub max_spread_bps: u32,
    
    /// Minimum fair price change to trigger update (basis points)
    pub min_fair_price_change_bps: u32,
    
    /// Order quantity in basis points (e.g., 10000 = 1 share)
    pub order_quantity_bp: i64,
    
    /// API credentials for order submission
    pub api_key: String,
    pub api_secret: String,
    
    /// WebSocket gateway URL
    pub websocket_gateway_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Cache TTL in seconds
    pub ttl_seconds: u64,
    
    /// Maximum cache size
    pub max_size: usize,
    
    /// How often to refresh cache (seconds)
    pub refresh_interval_seconds: u64,
}

impl Default for MarketMakerConfig {
    fn default() -> Self {
        Self {
            database: DatabaseConfig {
                url: "postgresql://postgres:password@localhost:5432/waiver_exchange".to_string(),
                max_connections: 10,
            },
            market_maker: MarketMakerParameters {
                enabled: true,
                spread_bps: 400, // ±4%
                update_frequency_seconds: 30,
                max_position_per_player: 100,
                daily_notional_limit_cents: 10_000_000, // $100k
                risk_check_interval_seconds: 60,
                max_spread_bps: 1200, // 12%
                min_fair_price_change_bps: 100, // 1%
                order_quantity_bp: 10000, // 1 share
                api_key: "ak_market_maker_1234567890abcdef".to_string(),
                api_secret: "sk_market_maker_abcdef1234567890".to_string(),
                websocket_gateway_url: "ws://localhost:8081/orders".to_string(),
            },
            cache: CacheConfig {
                ttl_seconds: 60, // 1 minute
                max_size: 1000,
                refresh_interval_seconds: 30, // 30 seconds
            },
        }
    }
}

impl MarketMakerConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> anyhow::Result<Self> {
        let mut config = Self::default();
        
        if let Ok(url) = std::env::var("DATABASE_URL") {
            config.database.url = url;
        }
        
        if let Ok(enabled) = std::env::var("MARKET_MAKER_ENABLED") {
            config.market_maker.enabled = enabled.parse().unwrap_or(true);
        }
        
        if let Ok(spread) = std::env::var("MARKET_MAKER_SPREAD_BPS") {
            config.market_maker.spread_bps = spread.parse().unwrap_or(400);
        }
        
        if let Ok(frequency) = std::env::var("MARKET_MAKER_UPDATE_FREQUENCY_SECONDS") {
            config.market_maker.update_frequency_seconds = frequency.parse().unwrap_or(30);
        }
        
        Ok(config)
    }
    
    /// Get update frequency as Duration
    pub fn update_frequency(&self) -> Duration {
        Duration::from_secs(self.market_maker.update_frequency_seconds)
    }
    
    /// Get cache TTL as Duration
    pub fn cache_ttl(&self) -> Duration {
        Duration::from_secs(self.cache.ttl_seconds)
    }
    
    /// Get cache refresh interval as Duration
    pub fn cache_refresh_interval(&self) -> Duration {
        Duration::from_secs(self.cache.refresh_interval_seconds)
    }
}
