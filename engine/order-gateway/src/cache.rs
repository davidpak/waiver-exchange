//! Redis caching module for performance optimization
//!
//! This module provides caching functionality for frequently accessed data
//! to improve API response times and reduce database load.

use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Client};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::debug;

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Redis connection URL
    pub redis_url: String,
    /// Default TTL for cached data
    pub default_ttl: Duration,
    /// TTL for price history data
    pub price_history_ttl: Duration,
    /// TTL for account data
    pub account_ttl: Duration,
    /// TTL for symbol info
    pub symbol_info_ttl: Duration,
    /// TTL for snapshot data
    pub snapshot_ttl: Duration,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            default_ttl: Duration::from_secs(60), // 1 minute
            price_history_ttl: Duration::from_secs(30), // 30 seconds
            account_ttl: Duration::from_secs(10), // 10 seconds
            symbol_info_ttl: Duration::from_secs(3600), // 1 hour
            snapshot_ttl: Duration::from_secs(1), // 1 second
        }
    }
}

/// Redis cache manager
pub struct CacheManager {
    connection_manager: ConnectionManager,
    config: CacheConfig,
}

impl CacheManager {
    /// Create a new cache manager
    pub async fn new(config: CacheConfig) -> Result<Self, redis::RedisError> {
        let client = Client::open(config.redis_url.as_str())?;
        let connection_manager = ConnectionManager::new(client).await?;

        Ok(Self { connection_manager, config })
    }

    /// Get cached data
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>, redis::RedisError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let mut conn = self.connection_manager.clone();
        let result: Option<String> = conn.get(key).await?;

        match result {
            Some(data) => {
                let deserialized: T = serde_json::from_str(&data).map_err(|e| {
                    redis::RedisError::from((
                        redis::ErrorKind::TypeError,
                        "JSON deserialization failed",
                        e.to_string(),
                    ))
                })?;
                debug!("Cache hit for key: {}", key);
                Ok(Some(deserialized))
            }
            None => {
                debug!("Cache miss for key: {}", key);
                Ok(None)
            }
        }
    }

    /// Set cached data with TTL
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Duration) -> Result<(), redis::RedisError>
    where
        T: Serialize,
    {
        let mut conn = self.connection_manager.clone();
        let serialized = serde_json::to_string(value).map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "JSON serialization failed",
                e.to_string(),
            ))
        })?;

        conn.set_ex(key, serialized, ttl.as_secs()).await?;
        debug!("Cached data for key: {} with TTL: {}s", key, ttl.as_secs());
        Ok(())
    }

    /// Delete cached data
    pub async fn delete(&self, key: &str) -> Result<(), redis::RedisError> {
        let mut conn = self.connection_manager.clone();
        conn.del(key).await?;
        debug!("Deleted cache key: {}", key);
        Ok(())
    }

    /// Cache price history data
    pub async fn cache_price_history<T>(
        &self,
        symbol_id: u32,
        period: &str,
        interval: &str,
        data: &T,
    ) -> Result<(), redis::RedisError>
    where
        T: Serialize,
    {
        let key = format!("price_history:{}:{}:{}", symbol_id, period, interval);
        self.set(&key, data, self.config.price_history_ttl).await
    }

    /// Get cached price history data
    pub async fn get_cached_price_history<T>(
        &self,
        symbol_id: u32,
        period: &str,
        interval: &str,
    ) -> Result<Option<T>, redis::RedisError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let key = format!("price_history:{}:{}:{}", symbol_id, period, interval);
        self.get(&key).await
    }

    /// Cache account summary data
    pub async fn cache_account_summary<T>(
        &self,
        account_id: i64,
        data: &T,
    ) -> Result<(), redis::RedisError>
    where
        T: Serialize,
    {
        let key = format!("account_summary:{}", account_id);
        self.set(&key, data, self.config.account_ttl).await
    }

    /// Get cached account summary data
    pub async fn get_cached_account_summary<T>(
        &self,
        account_id: i64,
    ) -> Result<Option<T>, redis::RedisError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let key = format!("account_summary:{}", account_id);
        self.get(&key).await
    }

    /// Cache symbol info data
    pub async fn cache_symbol_info<T>(
        &self,
        symbol_id: u32,
        data: &T,
    ) -> Result<(), redis::RedisError>
    where
        T: Serialize,
    {
        let key = format!("symbol_info:{}", symbol_id);
        self.set(&key, data, self.config.symbol_info_ttl).await
    }

    /// Get cached symbol info data
    pub async fn get_cached_symbol_info<T>(
        &self,
        symbol_id: u32,
    ) -> Result<Option<T>, redis::RedisError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let key = format!("symbol_info:{}", symbol_id);
        self.get(&key).await
    }

    /// Cache snapshot data
    pub async fn cache_snapshot<T>(&self, data: &T) -> Result<(), redis::RedisError>
    where
        T: Serialize,
    {
        let key = "snapshot:current";
        self.set(key, data, self.config.snapshot_ttl).await
    }

    /// Get cached snapshot data
    pub async fn get_cached_snapshot<T>(&self) -> Result<Option<T>, redis::RedisError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let key = "snapshot:current";
        self.get(key).await
    }

    /// Invalidate account cache (call when account data changes)
    pub async fn invalidate_account_cache(&self, account_id: i64) -> Result<(), redis::RedisError> {
        let key = format!("account_summary:{}", account_id);
        self.delete(&key).await
    }

    /// Invalidate symbol cache (call when symbol data changes)
    pub async fn invalidate_symbol_cache(&self, symbol_id: u32) -> Result<(), redis::RedisError> {
        let key = format!("symbol_info:{}", symbol_id);
        self.delete(&key).await
    }

    /// Invalidate price history cache (call when new trades occur)
    pub async fn invalidate_price_history_cache(
        &self,
        symbol_id: u32,
    ) -> Result<(), redis::RedisError> {
        // Delete all price history keys for this symbol
        let mut conn = self.connection_manager.clone();
        let pattern = format!("price_history:{}:*", symbol_id);
        let keys: Vec<String> = conn.keys(&pattern).await?;

        if !keys.is_empty() {
            let key_count = keys.len();
            conn.del(&keys).await?;
            debug!("Invalidated {} price history cache keys for symbol {}", key_count, symbol_id);
        }
        Ok(())
    }

    /// Invalidate snapshot cache (call when new orders/trades occur)
    pub async fn invalidate_snapshot_cache(&self) -> Result<(), redis::RedisError> {
        let key = "snapshot:current";
        self.delete(key).await
    }
}

/// Helper function to create cache manager with default config
pub async fn create_cache_manager() -> Result<CacheManager, redis::RedisError> {
    let config = CacheConfig::default();
    CacheManager::new(config).await
}
