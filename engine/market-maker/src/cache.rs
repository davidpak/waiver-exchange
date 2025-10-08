use crate::models::FairPrice;
use crate::config::CacheConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration};
use tracing::{info, debug};

/// Fair price cache with TTL
pub struct FairPriceCache {
    config: CacheConfig,
    cache: Arc<RwLock<HashMap<u32, CachedFairPrice>>>,
    last_refresh: Arc<RwLock<DateTime<Utc>>>,
}

#[derive(Debug, Clone)]
struct CachedFairPrice {
    fair_price: FairPrice,
    cached_at: DateTime<Utc>,
}

impl FairPriceCache {
    /// Create a new fair price cache
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
            last_refresh: Arc::new(RwLock::new(Utc::now() - Duration::hours(1))), // Start with old timestamp
        }
    }
    
    /// Get fair price for a symbol (from cache)
    pub async fn get_fair_price(&self, symbol_id: u32) -> Option<FairPrice> {
        let cache = self.cache.read().await;
        let cached = cache.get(&symbol_id)?;
        
        // Check if cache entry is still valid
        let now = Utc::now();
        let age = now - cached.cached_at;
        let ttl = chrono::Duration::seconds(self.config.ttl_seconds as i64);
        
        if age > ttl {
            debug!("Cache entry for symbol {} expired (age: {:?})", symbol_id, age);
            return None;
        }
        
        Some(cached.fair_price.clone())
    }
    
    /// Store fair price in cache
    pub async fn store_fair_price(&self, fair_price: FairPrice) {
        let mut cache = self.cache.write().await;
        
        // Check cache size limit
        if cache.len() >= self.config.max_size {
            // Remove oldest entries (simple LRU approximation)
            let mut entries: Vec<_> = cache.iter().map(|(k, v)| (*k, v.cached_at)).collect();
            entries.sort_by_key(|(_, cached_at)| *cached_at);
            
            let to_remove = entries.len() - self.config.max_size + 1;
            for (symbol_id, _) in entries.iter().take(to_remove) {
                cache.remove(symbol_id);
            }
            
            info!("Cache size limit reached, removed {} old entries", to_remove);
        }
        
        let cached = CachedFairPrice {
            fair_price: fair_price.clone(),
            cached_at: Utc::now(),
        };
        
        cache.insert(fair_price.symbol_id, cached);
        debug!("Cached fair price for symbol {}: {} cents", fair_price.symbol_id, fair_price.fair_cents);
    }
    
    /// Check if cache needs refresh
    pub async fn needs_refresh(&self) -> bool {
        let last_refresh = *self.last_refresh.read().await;
        let now = Utc::now();
        let refresh_interval = Duration::seconds(self.config.refresh_interval_seconds as i64);
        
        now - last_refresh > refresh_interval
    }
    
    /// Mark cache as refreshed
    pub async fn mark_refreshed(&self) {
        let mut last_refresh = self.last_refresh.write().await;
        *last_refresh = Utc::now();
    }
    
    /// Clear expired entries from cache
    pub async fn clear_expired(&self) {
        let mut cache = self.cache.write().await;
        let now = Utc::now();
        let ttl = Duration::seconds(self.config.ttl_seconds as i64);
        
        let initial_size = cache.len();
        cache.retain(|symbol_id, cached| {
            let age = now - cached.cached_at;
            if age > ttl {
                debug!("Removing expired cache entry for symbol {}", symbol_id);
                false
            } else {
                true
            }
        });
        
        let removed = initial_size - cache.len();
        if removed > 0 {
            info!("Cleared {} expired cache entries", removed);
        }
    }
    
    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let last_refresh = *self.last_refresh.read().await;
        
        CacheStats {
            size: cache.len(),
            max_size: self.config.max_size,
            ttl_seconds: self.config.ttl_seconds,
            last_refresh,
            oldest_entry: cache.values()
                .map(|cached| cached.cached_at)
                .min()
                .unwrap_or(Utc::now()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
    pub ttl_seconds: u64,
    pub last_refresh: DateTime<Utc>,
    pub oldest_entry: DateTime<Utc>,
}
