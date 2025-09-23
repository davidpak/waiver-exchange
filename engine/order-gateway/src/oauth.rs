use crate::error::{GatewayError, GatewayResult};
use account_service::account::Account;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

/// OAuth configuration
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub jwt_secret: String,
}

impl OAuthConfig {
    /// Create config from environment variables
    pub fn from_env() -> Result<Self, GatewayError> {
        let client_id = std::env::var("GOOGLE_CLIENT_ID")
            .map_err(|_| GatewayError::Authentication("GOOGLE_CLIENT_ID not set".to_string()))?;
        let client_secret = std::env::var("GOOGLE_CLIENT_SECRET").map_err(|_| {
            GatewayError::Authentication("GOOGLE_CLIENT_SECRET not set".to_string())
        })?;
        let redirect_url = std::env::var("GOOGLE_REDIRECT_URL")
            .map_err(|_| GatewayError::Authentication("GOOGLE_REDIRECT_URL not set".to_string()))?;
        let jwt_secret = std::env::var("JWT_SECRET")
            .map_err(|_| GatewayError::Authentication("JWT_SECRET not set".to_string()))?;

        Ok(Self { client_id, client_secret, redirect_url, jwt_secret })
    }
}

/// JWT claims for access tokens
#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,     // Account ID
    pub iss: String,     // Issuer
    pub aud: String,     // Audience
    pub exp: u64,        // Expiration time
    pub iat: u64,        // Issued at
    pub user_id: String, // Google user ID
    pub email: String,   // User email
    pub name: String,    // User display name
}

/// OAuth token response from Google
#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthResponse {
    pub access_token: String,
    pub id_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

/// Google user info - works with both OIDC and OAuth2 v2 endpoints
#[derive(Debug, Serialize, Deserialize)]
pub struct GoogleUserResult {
    #[serde(alias = "sub", alias = "id")]
    pub id: String,
    pub email: String,
    #[serde(alias = "email_verified", alias = "verified_email", default)]
    pub email_verified: bool,
    pub name: String,
    pub given_name: String,
    #[serde(default)]
    pub family_name: String,
    pub picture: String,
    #[serde(default)]
    pub locale: String,
}

/// OAuth token response for our API
#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub account_id: i64,
    pub user_info: GoogleUserResult,
}

/// OAuth manager for handling Google OAuth flow
pub struct OAuthManager {
    config: OAuthConfig,
    account_service: std::sync::Arc<account_service::AccountService>,
    http_client: Client,
    jwt_encoding_key: EncodingKey,
    jwt_decoding_key: DecodingKey,
}

impl OAuthManager {
    /// Create a new OAuth manager
    pub fn new(
        config: OAuthConfig,
        account_service: std::sync::Arc<account_service::AccountService>,
    ) -> Self {
        let http_client = Client::new();
        let jwt_encoding_key = EncodingKey::from_secret(config.jwt_secret.as_ref());
        let jwt_decoding_key = DecodingKey::from_secret(config.jwt_secret.as_ref());

        Self { config, account_service, http_client, jwt_encoding_key, jwt_decoding_key }
    }

    /// Generate OAuth authorization URL
    pub fn get_auth_url(&self) -> (String, String) {
        use oauth2::{
            basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, Scope, TokenUrl,
        };

        let client = BasicClient::new(
            ClientId::new(self.config.client_id.clone()),
            Some(ClientSecret::new(self.config.client_secret.clone())),
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap(), // v2 endpoint
            Some(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap()),
        )
        .set_redirect_uri(RedirectUrl::new(self.config.redirect_url.clone()).unwrap());

        let (auth_url, csrf_token) = client
            .authorize_url(oauth2::CsrfToken::new_random)
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("https://www.googleapis.com/auth/userinfo.email".to_string()))
            .add_scope(Scope::new("https://www.googleapis.com/auth/userinfo.profile".to_string()))
            .add_extra_param("prompt", "consent") // Force fresh consent
            .add_extra_param("include_granted_scopes", "true") // Include all granted scopes
            .url();

        info!("Generated OAuth URL: {}", auth_url);
        (auth_url.to_string(), csrf_token.secret().clone())
    }

    /// Exchange authorization code for tokens using direct HTTP request
    pub async fn exchange_code_for_tokens(
        &self,
        code: &str,
        _state: &str,
    ) -> GatewayResult<OAuthTokenResponse> {
        info!("Exchanging authorization code for tokens");

        // Exchange code for access token using direct HTTP request
        let oauth_response = self.request_token(code).await?;
        info!("Successfully exchanged code for access token");
        info!(
            "Access token (first 20 chars): {}",
            &oauth_response.access_token[..20.min(oauth_response.access_token.len())]
        );
        info!(
            "ID token (first 20 chars): {}",
            &oauth_response.id_token[..20.min(oauth_response.id_token.len())]
        );

        // Debug: Check token info to verify scopes
        self.debug_token_info(&oauth_response.access_token).await?;

        // Get user info from Google
        let user_info =
            self.get_google_user(&oauth_response.access_token, &oauth_response.id_token).await?;

        // Create or authenticate account (using user info we already have)
        let account = self
            .account_service
            .get_or_create_account_by_google_info(&user_info.id, &user_info.email, &user_info.name)
            .await
            .map_err(|e| {
                error!("Failed to get or create account: {}", e);
                GatewayError::Authentication(format!(
                    "Account creation/authentication failed: {}",
                    e
                ))
            })?;

        // Generate JWT access token
        let jwt_token = self.generate_jwt_token(&account, &user_info)?;

        info!(
            "OAuth authentication successful for user {} (account {})",
            user_info.email, account.id
        );

        Ok(OAuthTokenResponse { access_token: jwt_token, account_id: account.id as i64, user_info })
    }

    /// Request token from Google using direct HTTP request (based on working example)
    async fn request_token(&self, authorization_code: &str) -> GatewayResult<OAuthResponse> {
        let root_url = "https://oauth2.googleapis.com/token";
        let params = [
            ("grant_type", "authorization_code"),
            ("redirect_uri", self.config.redirect_url.as_str()),
            ("client_id", self.config.client_id.as_str()),
            ("code", authorization_code),
            ("client_secret", self.config.client_secret.as_str()),
        ];

        info!("Making token request to Google with params: grant_type=authorization_code, client_id={}", self.config.client_id);

        let response = self.http_client.post(root_url).form(&params).send().await.map_err(|e| {
            error!("Failed to make token request: {}", e);
            GatewayError::Authentication(format!("Token request failed: {}", e))
        })?;

        let status = response.status();
        let response_text = response.text().await.unwrap_or_default();

        info!("Google token response status: {}", status);
        info!("Google token response body: {}", response_text);

        if status.is_success() {
            let oauth_response: OAuthResponse =
                serde_json::from_str(&response_text).map_err(|e| {
                    error!("Failed to parse token response: {}", e);
                    error!("Response was: {}", response_text);
                    GatewayError::Authentication(format!("Failed to parse token response: {}", e))
                })?;
            Ok(oauth_response)
        } else {
            error!("Google token request failed: {} - {}", status, response_text);
            Err(GatewayError::Authentication(format!(
                "Google token request failed: {} - {}",
                status, response_text
            )))
        }
    }

    /// Debug method to check token info and verify scopes (non-blocking)
    async fn debug_token_info(&self, access_token: &str) -> GatewayResult<()> {
        info!("Debug: Checking token info to verify scopes");

        if let Ok(r) = self
            .http_client
            .get("https://oauth2.googleapis.com/tokeninfo")
            .query(&[("access_token", access_token)])
            .send()
            .await
        {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            info!("tokeninfo status={} body={}", status, body);
        } else {
            info!("tokeninfo check failed (ignored)");
        }
        Ok(())
    }

    /// Get user info from Google API using OIDC endpoint only
    async fn get_google_user(
        &self,
        access_token: &str,
        _id_token: &str,
    ) -> GatewayResult<GoogleUserResult> {
        use reqwest::header::WWW_AUTHENTICATE;

        info!("Requesting user info from Google OIDC API");
        info!(
            "Using access token (first 20 chars): {}",
            &access_token[..20.min(access_token.len())]
        );

        let resp = self
            .http_client
            .get("https://openidconnect.googleapis.com/v1/userinfo")
            .bearer_auth(access_token) // ACCESS token, not ID token
            .send()
            .await
            .map_err(|e| GatewayError::Authentication(format!("userinfo request error: {}", e)))?;

        let status = resp.status();
        if !status.is_success() {
            let www = resp
                .headers()
                .get(WWW_AUTHENTICATE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let body = resp.text().await.unwrap_or_default();
            return Err(GatewayError::Authentication(format!(
                "Google userinfo failed: status={} www-auth='{}' body='{}'",
                status, www, body
            )));
        }

        resp.json::<GoogleUserResult>()
            .await
            .map_err(|e| GatewayError::Authentication(format!("parse userinfo failed: {}", e)))
    }

    /// Generate JWT access token
    fn generate_jwt_token(
        &self,
        account: &Account,
        user_info: &GoogleUserResult,
    ) -> GatewayResult<String> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let claims = JwtClaims {
            sub: account.id.to_string(),
            iss: "waiver-exchange".to_string(),
            aud: "waiver-exchange-api".to_string(),
            exp: now + 3600, // 1 hour
            iat: now,
            user_id: user_info.id.clone(),
            email: user_info.email.clone(),
            name: user_info.name.clone(),
        };

        encode(&Header::new(Algorithm::HS256), &claims, &self.jwt_encoding_key).map_err(|e| {
            error!("Failed to generate JWT token: {}", e);
            GatewayError::Authentication(format!("JWT generation failed: {}", e))
        })
    }

    /// Validate JWT token
    pub fn validate_jwt_token(&self, token: &str) -> GatewayResult<JwtClaims> {
        let validation = Validation::new(Algorithm::HS256);
        let token_data =
            decode::<JwtClaims>(token, &self.jwt_decoding_key, &validation).map_err(|e| {
                warn!("JWT validation failed: {}", e);
                GatewayError::Authentication(format!("Invalid token: {}", e))
            })?;

        // Check if token is expired
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        if token_data.claims.exp < now {
            return Err(GatewayError::Authentication("Token expired".to_string()));
        }

        Ok(token_data.claims)
    }
}
