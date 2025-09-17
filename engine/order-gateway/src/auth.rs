//! Authentication module for the OrderGateway

use crate::error::GatewayError;
use crate::messages::{AuthRequest, AuthResponse, RateLimits};
use account_service::AccountService;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};

/// JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (Account ID)
    pub sub: String,
    /// Issuer
    pub iss: String,
    /// Audience
    pub aud: String,
    /// Expiration time
    pub exp: u64,
    /// Issued at
    pub iat: u64,
    /// Google user ID
    pub user_id: String,
    /// User email
    pub email: String,
    /// User name
    pub name: String,
}

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

    /// Account service for user-to-account mapping
    account_service: Arc<AccountService>,

    /// Active sessions (API key -> session info)
    sessions: Arc<RwLock<HashMap<String, crate::messages::UserSession>>>,
}

impl AuthManager {
    /// Create a new authentication manager
    pub fn new(account_service: Arc<AccountService>) -> Self {
        Self {
            store: Arc::new(ApiKeyStore::new()),
            account_service,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Authenticate a user and create a session
    pub async fn authenticate(&self, request: &AuthRequest) -> Result<AuthResponse, GatewayError> {
        let response = self.store.authenticate(request)?;

        if response.authenticated {
            let user_id = response.user_id.clone().unwrap_or_default();
            
            // Look up account ID for this user
            let account_id = self.account_service.get_account_id_by_user_id(&user_id)
                .await
                .map_err(|e| GatewayError::Authentication(format!("Failed to find account for user {}: {}", user_id, e)))?;

            // Create session with account ID
            let session = crate::messages::UserSession::new(
                user_id,
                account_id,
                response.permissions.clone(),
                response.rate_limits.clone(),
            );

            // Store session
            let mut sessions = self.sessions.write().await;
            sessions.insert(request.api_key.clone(), session);
        }

        Ok(response)
    }

    /// Get user session by API key
    pub async fn get_session(&self, api_key: &str) -> Result<crate::messages::UserSession, GatewayError> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(api_key).ok_or_else(|| {
            GatewayError::Authentication("Session not found".to_string())
        })?;

        Ok(session.clone())
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

    /// Authenticate using JWT token
    pub async fn authenticate_jwt(&self, token: &str) -> Result<AuthResponse, GatewayError> {
        // Get JWT secret from environment
        let jwt_secret = std::env::var("JWT_SECRET")
            .map_err(|_| GatewayError::Authentication("JWT_SECRET not configured".to_string()))?;

        // Decode and validate JWT token
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&["waiver-exchange-api"]); // Match the audience from OAuth server
        
        let token_data = decode::<JwtClaims>(
            token,
            &DecodingKey::from_secret(jwt_secret.as_ref()),
            &validation,
        ).map_err(|e| GatewayError::Authentication(format!("Invalid JWT token: {}", e)))?;

        let claims = token_data.claims;

        // Check if token is expired
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if claims.exp < now {
            return Err(GatewayError::Authentication("JWT token expired".to_string()));
        }

        // Parse account ID from subject (sub field contains the account ID)
        let account_id = claims.sub.parse::<i64>()
            .map_err(|e| GatewayError::Authentication(format!("Invalid account ID in JWT: {}", e)))?;

        // Create session with account ID from JWT claims
        let session = crate::messages::UserSession::new(
            claims.user_id.clone(),
            account_id,
            vec!["trade".to_string(), "market_data".to_string()], // Default permissions for OAuth users
            RateLimits {
                orders_per_second: 100,
                market_data_per_second: 1000,
                burst_limit: 10,
            },
        );

        // Store session using the JWT token as the key
        let mut sessions = self.sessions.write().await;
        sessions.insert(token.to_string(), session);

        Ok(AuthResponse {
            authenticated: true,
            user_id: Some(claims.user_id),
            permissions: vec!["trade".to_string(), "market_data".to_string()],
            rate_limits: RateLimits {
                orders_per_second: 100,
                market_data_per_second: 1000,
                burst_limit: 10,
            },
        })
    }

    /// Get user session by JWT token
    pub async fn get_session_by_jwt(&self, token: &str) -> Result<crate::messages::UserSession, GatewayError> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(token).ok_or_else(|| {
            GatewayError::Authentication("JWT session not found".to_string())
        })?;

        Ok(session.clone())
    }
}
