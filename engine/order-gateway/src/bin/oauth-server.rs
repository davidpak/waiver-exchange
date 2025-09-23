//! Simple OAuth server for handling Google OAuth flow
//! Runs on port 8082, separate from the main WebSocket server

use account_service::AccountService;
use order_gateway::oauth::{OAuthConfig, OAuthManager};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info};
use warp::Filter;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting OAuth server on port 8082...");

    // Load environment variables
    dotenv::dotenv().ok();

    // Initialize AccountService
    let account_service_config = account_service::AccountServiceConfig::from_env()
        .expect("Failed to load AccountService configuration");
    let account_service = Arc::new(
        AccountService::new(account_service_config).await.expect("Failed to create AccountService"),
    );

    // Initialize OAuth manager
    let oauth_config = OAuthConfig::from_env().expect("Failed to load OAuth configuration");
    let oauth_manager = Arc::new(OAuthManager::new(oauth_config, account_service));

    // OAuth routes
    let oauth_manager_1 = oauth_manager.clone();
    let oauth_manager_2 = oauth_manager.clone();

    // Google OAuth initiation route
    let auth_google = warp::path("auth")
        .and(warp::path("google"))
        .and(warp::get())
        .and(warp::query::<HashMap<String, String>>())
        .and_then(move |params: HashMap<String, String>| {
            let oauth_manager = oauth_manager_1.clone();
            async move {
                let default_state = String::new();
                let state = params.get("state").unwrap_or(&default_state);

                let (url, _csrf_token) = oauth_manager.get_auth_url();
                Ok::<_, warp::Rejection>(warp::reply::with_header(
                    warp::reply::with_status(
                        warp::reply::html("Redirecting to Google OAuth..."),
                        warp::http::StatusCode::FOUND,
                    ),
                    "Location",
                    url,
                ))
            }
        });

    // OAuth callback route
    let auth_callback = warp::path("auth")
        .and(warp::path("callback"))
        .and(warp::get())
        .and(warp::query::<HashMap<String, String>>())
        .and_then(move |params: HashMap<String, String>| {
            let oauth_manager = oauth_manager_2.clone();
            async move {
                let code = params
                    .get("code")
                    .ok_or_else(|| warp::reject::custom(OAuthError::MissingCode))?;
                let default_state = String::new();
                let state = params.get("state").unwrap_or(&default_state);

                match oauth_manager.exchange_code_for_tokens(code, state).await {
                    Ok(token_response) => {
                        // Return HTML page that closes popup and sends token to parent
                        let html = format!(
                            r#"<!DOCTYPE html>
                            <html>
                            <head><title>OAuth Success</title></head>
                            <body>
                                <script>
                                    if (window.opener) {{
                                        window.opener.postMessage({{
                                            type: 'oauth_success',
                                            token: '{}'
                                        }}, '*');
                                        window.close();
                                    }}
                                </script>
                                <p>Authentication successful! You can close this window.</p>
                            </body>
                            </html>"#,
                            token_response.access_token
                        );
                        Ok::<_, warp::Rejection>(warp::reply::html(html))
                    }
                    Err(e) => {
                        error!("OAuth callback failed: {}", e);
                        Err(warp::reject::custom(OAuthError::OAuthFailed))
                    }
                }
            }
        });

    // Health check route
    let health = warp::path("health").and(warp::get()).map(|| {
        warp::reply::json(&serde_json::json!({
            "status": "healthy",
            "service": "oauth-server"
        }))
    });

    // Combine routes
    let routes = auth_google.or(auth_callback).or(health).with(
        warp::cors()
            .allow_any_origin()
            .allow_headers(vec!["content-type"])
            .allow_methods(vec!["GET", "POST"]),
    );

    // Start server on port 8082
    let addr = ([0, 0, 0, 0], 8082);
    info!("OAuth server listening on http://0.0.0.0:8082");

    warp::serve(routes).run(addr).await;
}

/// OAuth errors for warp rejections
#[derive(Debug)]
pub enum OAuthError {
    MissingCode,
    OAuthFailed,
}

impl warp::reject::Reject for OAuthError {}
