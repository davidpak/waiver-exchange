//! Google OAuth integration

use crate::config::OAuthConfig;
use crate::AccountServiceError;
use oauth2::{basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl, TokenResponse};
use serde::Deserialize;

/// Google OAuth client
#[derive(Debug)]
pub struct GoogleOAuthClient {
    client: BasicClient,
}

/// Google user info response
#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
    pub id: String,
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
}

impl GoogleOAuthClient {
    /// Create a new Google OAuth client
    pub fn new(config: &OAuthConfig) -> Result<Self, AccountServiceError> {
        let client = BasicClient::new(
            ClientId::new(config.client_id.clone()),
            Some(ClientSecret::new(config.client_secret.clone())),
            AuthUrl::new("https://accounts.google.com/o/oauth2/auth".to_string())
                .map_err(|e| AccountServiceError::InvalidConfig {
                    message: format!("Invalid auth URL: {}", e),
                })?,
            Some(TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
                .map_err(|e| AccountServiceError::InvalidConfig {
                    message: format!("Invalid token URL: {}", e),
                })?),
        )
        .set_redirect_uri(RedirectUrl::new(config.redirect_url.clone())
            .map_err(|e| AccountServiceError::InvalidConfig {
                message: format!("Invalid redirect URL: {}", e),
            })?);

        Ok(Self { client })
    }
    
    /// Get authorization URL
    pub fn get_authorization_url(&self) -> String {
        let (auth_url, _) = self.client.authorize_url(oauth2::CsrfToken::new_random).url();
        auth_url.to_string()
    }
    
    /// Exchange authorization code for access token
    pub async fn exchange_code(&self, code: &str) -> Result<String, AccountServiceError> {
        let token = self
            .client
            .exchange_code(oauth2::AuthorizationCode::new(code.to_string()))
            .request_async(oauth2::reqwest::async_http_client)
            .await?;
            
        Ok(token.access_token().secret().clone())
    }
    
    /// Get user info from access token
    pub async fn get_user_info(&self, access_token: &str) -> Result<GoogleUserInfo, AccountServiceError> {
        let client = reqwest::Client::new();
        let response = client
            .get("https://www.googleapis.com/oauth2/v2/userinfo")
            .bearer_auth(access_token)
            .send()
            .await?;
            
        if !response.status().is_success() {
            return Err(AccountServiceError::GoogleOAuthError {
                message: format!("Failed to get user info: {}", response.status()),
            });
        }
        
        let user_info: GoogleUserInfo = response.json().await?;
        Ok(user_info)
    }
}
