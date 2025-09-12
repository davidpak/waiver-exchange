//! Authentication module for the OrderGateway

use crate::error::GatewayError;
use crate::messages::{AuthRequest, AuthResponse, RateLimits};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Simple in-memory API key store
/// In production, this would connect to a proper authentication service
#[derive(Debug, Clone)]
pub struct ApiKeyStore {
    /// Map of API key to user information
    keys: HashMap<String, UserInfo>,
}

/// User information stored in the API key store
#[derive(Debug, Clone)]
pub struct UserInfo {
    /// User ID
    pub user_id: String,

    /// API secret (for validation)
    pub api_secret: String,

    /// User permissions
    pub permissions: Vec<String>,

    /// Rate limits
    pub rate_limits: RateLimits,
}

impl Default for ApiKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiKeyStore {
    /// Create a new API key store
    pub fn new() -> Self {
        let mut keys = HashMap::new();

        // Add some test API keys for development
        keys.insert(
            "ak_test_1234567890abcdef".to_string(),
            UserInfo {
                user_id: "user123".to_string(),
                api_secret: "sk_test_abcdef1234567890".to_string(),
                permissions: vec!["trade".to_string(), "market_data".to_string()],
                rate_limits: RateLimits {
                    orders_per_second: 100,
                    market_data_per_second: 1000,
                    burst_limit: 10,
                },
            },
        );

        keys.insert(
            "ak_admin_abcdef1234567890".to_string(),
            UserInfo {
                user_id: "admin".to_string(),
                api_secret: "sk_admin_1234567890abcdef".to_string(),
                permissions: vec![
                    "trade".to_string(),
                    "market_data".to_string(),
                    "admin".to_string(),
                ],
                rate_limits: RateLimits {
                    orders_per_second: 1000,
                    market_data_per_second: 10000,
                    burst_limit: 100,
                },
            },
        );

        Self { keys }
    }

    /// Authenticate a user with API key and secret
    #[allow(clippy::result_large_err)]
    pub fn authenticate(&self, request: &AuthRequest) -> Result<AuthResponse, GatewayError> {
        let user_info = self
            .keys
            .get(&request.api_key)
            .ok_or_else(|| GatewayError::Authentication("Invalid API key".to_string()))?;

        if user_info.api_secret != request.api_secret {
            return Err(GatewayError::Authentication("Invalid API secret".to_string()));
        }

        Ok(AuthResponse {
            authenticated: true,
            user_id: Some(user_info.user_id.clone()),
            permissions: user_info.permissions.clone(),
            rate_limits: user_info.rate_limits.clone(),
        })
    }

    /// Get user information by API key
    pub fn get_user_info(&self, api_key: &str) -> Option<&UserInfo> {
        self.keys.get(api_key)
    }
}

/// Authentication manager
#[derive(Debug)]
pub struct AuthManager {
    /// API key store
    store: Arc<ApiKeyStore>,

    /// Active sessions (API key -> session info)
    sessions: Arc<RwLock<HashMap<String, crate::messages::UserSession>>>,
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthManager {
    /// Create a new authentication manager
    pub fn new() -> Self {
        Self {
            store: Arc::new(ApiKeyStore::new()),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Authenticate a user and create a session
    pub async fn authenticate(&self, request: &AuthRequest) -> Result<AuthResponse, GatewayError> {
        let response = self.store.authenticate(request)?;

        if response.authenticated {
            // Create session
            let session = crate::messages::UserSession::new(
                response.user_id.clone().unwrap_or_default(),
                response.permissions.clone(),
                response.rate_limits.clone(),
            );

            // Store session
            let mut sessions = self.sessions.write().await;
            sessions.insert(request.api_key.clone(), session);
        }

        Ok(response)
    }

    /// Validate an API key and return user session
    pub async fn validate_session(
        &self,
        api_key: &str,
    ) -> Result<crate::messages::UserSession, GatewayError> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(api_key).ok_or_else(|| {
            GatewayError::Authentication("Invalid or expired session".to_string())
        })?;

        // Update last activity
        let mut session = session.clone();
        session.update_activity();

        Ok(session)
    }

    /// Remove a session
    pub async fn remove_session(&self, api_key: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(api_key);
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self, max_age_seconds: u64) {
        let mut sessions = self.sessions.write().await;
        let cutoff = std::time::Instant::now() - std::time::Duration::from_secs(max_age_seconds);

        sessions.retain(|_, session| session.last_activity > cutoff);
    }
}
