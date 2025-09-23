//! Configuration for AccountService

use serde::{Deserialize, Serialize};

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
}

/// Google OAuth configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
}

/// Sleeper API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SleeperConfig {
    pub api_base_url: String,
    pub api_key: Option<String>,
}

/// AccountService configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountServiceConfig {
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub oauth: OAuthConfig,
    pub sleeper: SleeperConfig,
    pub fantasy_points_conversion_rate: u32,
    pub reservation_expiry_days: u32,
    pub cache_ttl_seconds: u32,
}

impl Default for AccountServiceConfig {
    fn default() -> Self {
        Self {
            database: DatabaseConfig {
                url: "postgresql://user:pass@localhost/waiver_exchange".to_string(),
                max_connections: 10,
                min_connections: 1,
            },
            redis: RedisConfig { url: "redis://localhost:6379".to_string() },
            oauth: OAuthConfig {
                client_id: "".to_string(),
                client_secret: "".to_string(),
                redirect_url: "".to_string(),
            },
            sleeper: SleeperConfig {
                api_base_url: "https://api.sleeper.app/v1".to_string(),
                api_key: None,
            },
            fantasy_points_conversion_rate: 1000, // $10 per fantasy point (1000 cents)
            reservation_expiry_days: 7,
            cache_ttl_seconds: 300, // 5 minutes
        }
    }
}

impl AccountServiceConfig {
    /// Create config from environment variables
    pub fn from_env() -> Result<Self, crate::AccountServiceError> {
        // Environment variables should be set by the deployment platform
        let database_url = std::env::var("DATABASE_URL").map_err(|_| {
            crate::AccountServiceError::InvalidConfig {
                message: "DATABASE_URL not set".to_string(),
            }
        })?;

        let redis_url = std::env::var("REDIS_URL").map_err(|_| {
            crate::AccountServiceError::InvalidConfig { message: "REDIS_URL not set".to_string() }
        })?;

        let google_client_id = std::env::var("GOOGLE_CLIENT_ID").map_err(|_| {
            crate::AccountServiceError::InvalidConfig {
                message: "GOOGLE_CLIENT_ID not set".to_string(),
            }
        })?;

        let google_client_secret = std::env::var("GOOGLE_CLIENT_SECRET").map_err(|_| {
            crate::AccountServiceError::InvalidConfig {
                message: "GOOGLE_CLIENT_SECRET not set".to_string(),
            }
        })?;

        let google_redirect_url = std::env::var("GOOGLE_REDIRECT_URL").map_err(|_| {
            crate::AccountServiceError::InvalidConfig {
                message: "GOOGLE_REDIRECT_URL not set".to_string(),
            }
        })?;

        let sleeper_api_base_url = std::env::var("SLEEPER_API_BASE_URL")
            .unwrap_or_else(|_| "https://api.sleeper.app/v1".to_string());

        let sleeper_api_key = std::env::var("SLEEPER_API_KEY").ok();

        let fantasy_points_conversion_rate = std::env::var("FANTASY_POINTS_CONVERSION_RATE")
            .unwrap_or_else(|_| "1000".to_string())
            .parse::<u32>()
            .map_err(|_| crate::AccountServiceError::InvalidConfig {
                message: "Invalid FANTASY_POINTS_CONVERSION_RATE".to_string(),
            })?;

        let reservation_expiry_days = std::env::var("RESERVATION_EXPIRY_DAYS")
            .unwrap_or_else(|_| "7".to_string())
            .parse::<u32>()
            .map_err(|_| crate::AccountServiceError::InvalidConfig {
                message: "Invalid RESERVATION_EXPIRY_DAYS".to_string(),
            })?;

        let cache_ttl_seconds = std::env::var("CACHE_TTL_SECONDS")
            .unwrap_or_else(|_| "300".to_string())
            .parse::<u32>()
            .map_err(|_| crate::AccountServiceError::InvalidConfig {
                message: "Invalid CACHE_TTL_SECONDS".to_string(),
            })?;

        Ok(Self {
            database: DatabaseConfig { url: database_url, max_connections: 10, min_connections: 1 },
            redis: RedisConfig { url: redis_url },
            oauth: OAuthConfig {
                client_id: google_client_id,
                client_secret: google_client_secret,
                redirect_url: google_redirect_url,
            },
            sleeper: SleeperConfig { api_base_url: sleeper_api_base_url, api_key: sleeper_api_key },
            fantasy_points_conversion_rate,
            reservation_expiry_days,
            cache_ttl_seconds,
        })
    }
}
