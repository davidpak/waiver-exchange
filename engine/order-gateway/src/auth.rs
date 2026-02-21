//! Authentication module for the OrderGateway

use crate::error::GatewayError;
use crate::messages::{AuthRequest, AuthResponse, RateLimits};
use account_service::AccountService;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

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

/// Supabase JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct SupabaseJwtClaims {
    /// Subject (Supabase user UUID)
    pub sub: String,
    /// Audience (should be "authenticated")
    pub aud: String,
    /// Expiration time
    pub exp: u64,
    /// Issued at
    pub iat: u64,
    /// Role (e.g., "authenticated")
    pub role: Option<String>,
    /// Email
    pub email: Option<String>,
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

        // Add API key for account 7 (used in EVS integration tests)
        keys.insert(
            "ak_test_7_abcdef1234567890".to_string(),
            UserInfo {
                user_id: "user7".to_string(),
                api_secret: "sk_test_7_1234567890abcdef".to_string(),
                permissions: vec!["trade".to_string(), "market_data".to_string()],
                rate_limits: RateLimits {
                    orders_per_second: 100,
                    market_data_per_second: 1000,
                    burst_limit: 10,
                },
            },
        );

        // Add API key for market maker bot (house account)
        keys.insert(
            "ak_market_maker_1234567890abcdef".to_string(),
            UserInfo {
                user_id: "market_maker_bot".to_string(),
                api_secret: "sk_market_maker_abcdef1234567890".to_string(),
                permissions: vec!["trade".to_string(), "market_data".to_string()],
                rate_limits: RateLimits {
                    orders_per_second: 1000, // High rate limit for market making
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
pub struct AuthManager {
    /// API key store
    store: Arc<ApiKeyStore>,

    /// Account service for user-to-account mapping
    account_service: Arc<AccountService>,

    /// Active sessions (API key -> session info)
    sessions: Arc<RwLock<HashMap<String, crate::messages::UserSession>>>,

    /// Cached Supabase JWKS decoding keys (fetched at startup)
    supabase_jwks: Arc<RwLock<Vec<(Algorithm, DecodingKey)>>>,
}

impl AuthManager {
    /// Create a new authentication manager
    pub fn new(account_service: Arc<AccountService>) -> Self {
        let manager = Self {
            store: Arc::new(ApiKeyStore::new()),
            account_service,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            supabase_jwks: Arc::new(RwLock::new(Vec::new())),
        };

        // Fetch JWKS in background
        let jwks = manager.supabase_jwks.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::fetch_supabase_jwks(jwks).await {
                warn!("Failed to fetch Supabase JWKS: {}. Will retry on first auth attempt.", e);
            }
        });

        manager
    }

    /// Fetch and cache Supabase JWKS public keys
    async fn fetch_supabase_jwks(
        jwks_cache: Arc<RwLock<Vec<(Algorithm, DecodingKey)>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let supabase_url = std::env::var("NEXT_PUBLIC_SUPABASE_URL")
            .or_else(|_| std::env::var("SUPABASE_URL"))
            .map_err(|_| "NEXT_PUBLIC_SUPABASE_URL or SUPABASE_URL not set")?;

        let jwks_url = format!("{}/auth/v1/.well-known/jwks.json", supabase_url);
        info!("Fetching Supabase JWKS from {}", jwks_url);

        let resp = reqwest::get(&jwks_url).await?;
        let jwks: jsonwebtoken::jwk::JwkSet = resp.json().await?;

        let mut keys = Vec::new();
        for jwk in &jwks.keys {
            let alg = match jwk.common.key_algorithm {
                Some(jsonwebtoken::jwk::KeyAlgorithm::ES256) => Algorithm::ES256,
                Some(jsonwebtoken::jwk::KeyAlgorithm::HS256) => Algorithm::HS256,
                Some(jsonwebtoken::jwk::KeyAlgorithm::RS256) => Algorithm::RS256,
                _ => {
                    // Try to infer from key type
                    match &jwk.algorithm {
                        jsonwebtoken::jwk::AlgorithmParameters::EllipticCurve(_) => Algorithm::ES256,
                        jsonwebtoken::jwk::AlgorithmParameters::RSA(_) => Algorithm::RS256,
                        jsonwebtoken::jwk::AlgorithmParameters::OctetKey(_) => Algorithm::HS256,
                        _ => continue,
                    }
                }
            };

            match DecodingKey::from_jwk(jwk) {
                Ok(key) => {
                    info!("Loaded Supabase JWK: kid={:?}, alg={:?}", jwk.common.key_id, alg);
                    keys.push((alg, key));
                }
                Err(e) => {
                    warn!("Failed to parse Supabase JWK: {}", e);
                }
            }
        }

        info!("Cached {} Supabase JWKS keys", keys.len());
        let mut cache = jwks_cache.write().await;
        *cache = keys;
        Ok(())
    }

    /// Authenticate a user and create a session
    pub async fn authenticate(&self, request: &AuthRequest) -> Result<AuthResponse, GatewayError> {
        let response = self.store.authenticate(request)?;

        if response.authenticated {
            let user_id = response.user_id.clone().unwrap_or_default();

            // Look up account ID for this user
            let account_id =
                self.account_service.get_account_id_by_user_id(&user_id).await.map_err(|e| {
                    GatewayError::Authentication(format!(
                        "Failed to find account for user {}: {}",
                        user_id, e
                    ))
                })?;

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
    pub async fn get_session(
        &self,
        api_key: &str,
    ) -> Result<crate::messages::UserSession, GatewayError> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(api_key)
            .ok_or_else(|| GatewayError::Authentication("Session not found".to_string()))?;

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

    /// Authenticate using JWT token (supports both legacy custom JWTs and Supabase JWTs)
    pub async fn authenticate_jwt(&self, token: &str) -> Result<AuthResponse, GatewayError> {
        // Try Supabase JWT first, then fall back to legacy JWT
        match self.authenticate_supabase_jwt(token).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                tracing::warn!("Supabase JWT auth failed, trying legacy: {}", e);
            }
        }

        // Legacy JWT validation
        let jwt_secret = std::env::var("JWT_SECRET")
            .map_err(|_| GatewayError::Authentication("JWT_SECRET not configured".to_string()))?;

        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&["waiver-exchange-api"]);

        let token_data =
            decode::<JwtClaims>(token, &DecodingKey::from_secret(jwt_secret.as_ref()), &validation)
                .map_err(|e| GatewayError::Authentication(format!("Invalid JWT token: {}", e)))?;

        let claims = token_data.claims;

        let now =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();

        if claims.exp < now {
            return Err(GatewayError::Authentication("JWT token expired".to_string()));
        }

        let account_id = claims.sub.parse::<i64>().map_err(|e| {
            GatewayError::Authentication(format!("Invalid account ID in JWT: {}", e))
        })?;

        let session = crate::messages::UserSession::new(
            claims.user_id.clone(),
            account_id,
            vec!["trade".to_string(), "market_data".to_string()],
            RateLimits { orders_per_second: 100, market_data_per_second: 1000, burst_limit: 10 },
        );

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

    /// Authenticate using a Supabase JWT token
    async fn authenticate_supabase_jwt(&self, token: &str) -> Result<AuthResponse, GatewayError> {
        // Try cached JWKS keys first (supports ES256, RS256, HS256)
        let jwks = self.supabase_jwks.read().await;

        // If JWKS cache is empty, try fetching again
        if jwks.is_empty() {
            drop(jwks);
            let _ = Self::fetch_supabase_jwks(self.supabase_jwks.clone()).await;
            // Re-acquire read lock
            let jwks = self.supabase_jwks.read().await;
            if jwks.is_empty() {
                return Err(GatewayError::Authentication(
                    "No Supabase JWKS keys available".to_string(),
                ));
            }
            return self.try_jwks_auth(token, &jwks).await;
        }

        self.try_jwks_auth(token, &jwks).await
    }

    /// Try authenticating with each cached JWKS key
    async fn try_jwks_auth(
        &self,
        token: &str,
        jwks: &[(Algorithm, DecodingKey)],
    ) -> Result<AuthResponse, GatewayError> {
        let mut last_error = None;

        for (alg, key) in jwks {
            let mut validation = Validation::new(*alg);
            validation.set_audience(&["authenticated"]);
            validation.validate_aud = true;

            match decode::<SupabaseJwtClaims>(token, key, &validation) {
                Ok(token_data) => {
                    return self.process_supabase_claims(token, token_data.claims).await;
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        // Also try HS256 with legacy secret as fallback
        if let Ok(secret) = std::env::var("SUPABASE_JWT_SECRET") {
            let mut validation = Validation::new(Algorithm::HS256);
            validation.set_audience(&["authenticated"]);
            validation.validate_aud = true;

            if let Ok(token_data) = decode::<SupabaseJwtClaims>(
                token,
                &DecodingKey::from_secret(secret.as_ref()),
                &validation,
            ) {
                return self.process_supabase_claims(token, token_data.claims).await;
            }
        }

        Err(GatewayError::Authentication(format!(
            "Invalid Supabase JWT: {}",
            last_error.map(|e| e.to_string()).unwrap_or_else(|| "no keys available".to_string())
        )))
    }

    /// Process validated Supabase JWT claims into an auth response
    async fn process_supabase_claims(
        &self,
        token: &str,
        claims: SupabaseJwtClaims,
    ) -> Result<AuthResponse, GatewayError> {
        let now =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();

        if claims.exp < now {
            return Err(GatewayError::Authentication("Supabase JWT expired".to_string()));
        }

        // Look up account by supabase_uid (the sub claim is the Supabase user UUID)
        let account_id = self
            .account_service
            .get_account_by_supabase_uid(&claims.sub)
            .await
            .map_err(|e| {
                GatewayError::Authentication(format!(
                    "No account found for Supabase user {}: {}",
                    claims.sub, e
                ))
            })?;

        let user_id_str = account_id.to_string();

        let session = crate::messages::UserSession::new(
            user_id_str.clone(),
            account_id,
            vec!["trade".to_string(), "market_data".to_string()],
            RateLimits { orders_per_second: 100, market_data_per_second: 1000, burst_limit: 10 },
        );

        let mut sessions = self.sessions.write().await;
        sessions.insert(token.to_string(), session);

        Ok(AuthResponse {
            authenticated: true,
            user_id: Some(user_id_str),
            permissions: vec!["trade".to_string(), "market_data".to_string()],
            rate_limits: RateLimits {
                orders_per_second: 100,
                market_data_per_second: 1000,
                burst_limit: 10,
            },
        })
    }

    /// Get user session by JWT token
    pub async fn get_session_by_jwt(
        &self,
        token: &str,
    ) -> Result<crate::messages::UserSession, GatewayError> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(token)
            .ok_or_else(|| GatewayError::Authentication("JWT session not found".to_string()))?;

        Ok(session.clone())
    }
}
