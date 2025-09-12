//! Rate limiting for the OrderGateway

use crate::config::RateLimitConfig;
use crate::error::GatewayError;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Simple rate limiter for order submissions and market data
pub struct RateLimiter {
    /// Order submission rate limiter
    order_limiter: Arc<RwLock<HashMap<String, Vec<Instant>>>>,

    /// Market data rate limiter
    market_data_limiter: Arc<RwLock<HashMap<String, Vec<Instant>>>>,

    /// Configuration
    config: RateLimitConfig,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            order_limiter: Arc::new(RwLock::new(HashMap::new())),
            market_data_limiter: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Check if an order submission is allowed
    pub async fn check_order_rate_limit(&self, user_id: &str) -> Result<(), GatewayError> {
        let now = Instant::now();
        let window = Duration::from_secs(self.config.window_seconds);

        let mut limiter = self.order_limiter.write().await;
        let user_requests = limiter.entry(user_id.to_string()).or_insert_with(Vec::new);

        // Remove old requests outside the window
        user_requests.retain(|&time| now.duration_since(time) < window);

        // Check if we're within the rate limit
        if user_requests.len() >= self.config.orders_per_second as usize {
            return Err(GatewayError::RateLimit("Order rate limit exceeded".to_string()));
        }

        // Add current request
        user_requests.push(now);
        Ok(())
    }

    /// Check if market data access is allowed
    pub async fn check_market_data_rate_limit(&self, user_id: &str) -> Result<(), GatewayError> {
        let now = Instant::now();
        let window = Duration::from_secs(self.config.window_seconds);

        let mut limiter = self.market_data_limiter.write().await;
        let user_requests = limiter.entry(user_id.to_string()).or_insert_with(Vec::new);

        // Remove old requests outside the window
        user_requests.retain(|&time| now.duration_since(time) < window);

        // Check if we're within the rate limit
        if user_requests.len() >= self.config.market_data_per_second as usize {
            return Err(GatewayError::RateLimit("Market data rate limit exceeded".to_string()));
        }

        // Add current request
        user_requests.push(now);
        Ok(())
    }

    /// Get rate limit information for a user
    pub fn get_rate_limit_info(&self, _user_id: &str) -> RateLimitInfo {
        // For now, return static information
        // In a real implementation, you'd query the current state
        RateLimitInfo {
            orders_per_second: self.config.orders_per_second,
            market_data_per_second: self.config.market_data_per_second,
            burst_limit: self.config.burst_limit,
            window_seconds: self.config.window_seconds,
        }
    }
}

/// Rate limit information for a user
#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    /// Orders per second
    pub orders_per_second: u32,

    /// Market data updates per second
    pub market_data_per_second: u32,

    /// Burst limit
    pub burst_limit: u32,

    /// Window in seconds
    pub window_seconds: u64,
}
