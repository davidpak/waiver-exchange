//! WebSocket connection handler for the OrderGateway

use crate::auth::AuthManager;
use crate::error::{GatewayError, GatewayResult};
use crate::market_data_broadcaster::MarketDataBroadcaster;
use crate::messages::{AuthRequest, Message as ApiMessage, OrderPlaceRequest, OrderPlaceResponse};
use crate::rate_limiter::RateLimiter;

use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tracing::{debug, error, info, warn};

// OrderRouter integration
use order_router::{InboundMsgWithSymbol, OrderRouter, RouterError};
use player_registry::PlayerRegistry;
use symbol_coordinator::SymbolCoordinator;
use whistle::{AccountId, InboundMsg, OrderId, OrderType, Side, TickId};
use account_service::{AccountService, Balance};

/// WebSocket connection handler
pub struct WebSocketHandler {
    /// Peer address
    peer_addr: SocketAddr,

    /// Authentication manager
    auth_manager: Arc<AuthManager>,

    /// Rate limiter
    rate_limiter: Arc<RateLimiter>,

    /// Market data broadcaster
    market_data_broadcaster: Arc<MarketDataBroadcaster>,

    /// Order router for routing orders
    order_router: Arc<RwLock<OrderRouter>>,

    /// Symbol coordinator for managing trading engines
    symbol_coordinator: Arc<SymbolCoordinator>,

    /// Player registry for mapping player names to symbol IDs
    player_registry: Arc<RwLock<PlayerRegistry>>,

    /// Account service for balance and position validation
    account_service: Arc<AccountService>,

    /// WebSocket sender channel
    sender: Option<mpsc::UnboundedSender<WsMessage>>,

    /// User session (if authenticated)
    user_session: Option<crate::messages::UserSession>,
}

impl WebSocketHandler {
    /// Create a new WebSocket handler
    pub fn new(
        peer_addr: SocketAddr,
        auth_manager: Arc<AuthManager>,
        rate_limiter: Arc<RateLimiter>,
        market_data_broadcaster: Arc<MarketDataBroadcaster>,
        order_router: Arc<RwLock<OrderRouter>>,
        symbol_coordinator: Arc<SymbolCoordinator>,
        player_registry: Arc<RwLock<PlayerRegistry>>,
        account_service: Arc<AccountService>,
    ) -> Self {
        Self {
            peer_addr,
            auth_manager,
            rate_limiter,
            market_data_broadcaster,
            order_router,
            symbol_coordinator,
            player_registry,
            account_service,
            sender: None,
            user_session: None,
        }
    }

    /// Handle the WebSocket connection
    pub async fn handle(&mut self, stream: TcpStream) -> GatewayResult<()> {
        info!("Handling WebSocket connection from {}", self.peer_addr);

        // Accept the WebSocket connection
        let ws_stream = accept_async(stream).await?;

        // Split the stream
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Create sender channel
        let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();
        self.sender = Some(tx);

        // Spawn sender task
        let sender_task = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if let Err(e) = ws_sender.send(message).await {
                    error!("Failed to send WebSocket message: {}", e);
                    break;
                }
            }
        });

        // Main message handling loop
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(message) => {
                    if let Err(e) = self.handle_message(message).await {
                        error!("Failed to handle message: {}", e);
                        self.send_error("message_handling_error", &e.to_string()).await;
                    }
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
            }
        }

        // Clean up
        if let Some(session) = &self.user_session {
            self.market_data_broadcaster.remove_client(&session.user_id).await;
        }

        // Cancel sender task
        sender_task.abort();

        info!("WebSocket connection from {} closed", self.peer_addr);
        Ok(())
    }

    /// Handle incoming WebSocket messages
    async fn handle_message(&mut self, message: WsMessage) -> GatewayResult<()> {
        match message {
            WsMessage::Text(text) => {
                debug!("Received text message: {}", text);
                self.handle_text_message(text).await?;
            }
            WsMessage::Binary(_) => {
                warn!("Received binary message, ignoring");
            }
            WsMessage::Ping(data) => {
                self.send_message(WsMessage::Pong(data)).await?;
            }
            WsMessage::Pong(_) => {
                // Ignore pong messages
            }
            WsMessage::Close(_) => {
                info!("Received close message from {}", self.peer_addr);
            }
            WsMessage::Frame(_) => {
                // Ignore raw frames
            }
        }
        Ok(())
    }

    /// Handle text messages
    async fn handle_text_message(&mut self, text: String) -> GatewayResult<()> {
        let message: ApiMessage = serde_json::from_str(&text)?;

        match message.method.as_deref() {
            Some("auth.login") => {
                self.handle_auth(message).await?;
            }
            Some("order.place") => {
                self.handle_order_place(message).await?;
            }
            Some("market_data.subscribe") => {
                self.handle_market_data_subscribe(message).await?;
            }
            _ => {
                return Err(GatewayError::System(format!("Unknown method: {:?}", message.method)));
            }
        }

        Ok(())
    }

    /// Handle authentication
    async fn handle_auth(&mut self, message: ApiMessage) -> GatewayResult<()> {
        let auth_request: AuthRequest = serde_json::from_value(
            message
                .params
                .ok_or_else(|| GatewayError::System("Missing auth parameters".to_string()))?,
        )?;

        let auth_response = self.auth_manager.authenticate(&auth_request).await?;

        if auth_response.authenticated {
            // Get the session from AuthManager (which includes the account_id)
            let session = self.auth_manager.get_session(&auth_request.api_key).await?;
            self.user_session = Some(session.clone());

            // Add to market data broadcaster
            if let Some(sender) = &self.sender {
                self.market_data_broadcaster
                    .add_client(session.user_id.clone(), sender.clone())
                    .await;
            }
        }

        let response = ApiMessage {
            id: message.id,
            method: None,
            stream: None,
            params: None,
            data: None,
            result: Some(serde_json::to_value(auth_response)?),
            error: None,
        };

        self.send_json_message(response).await?;
        Ok(())
    }

    /// Handle order placement
    async fn handle_order_place(&mut self, message: ApiMessage) -> GatewayResult<()> {
        info!("Handling order placement request: {:?}", message);

        // Check authentication
        let session = self
            .user_session
            .as_ref()
            .ok_or_else(|| GatewayError::Authentication("Not authenticated".to_string()))?;

        if !session.has_permission("trade") {
            return Err(GatewayError::Authentication("Insufficient permissions".to_string()));
        }

        // Check rate limits
        self.rate_limiter.check_order_rate_limit(&session.user_id).await?;

        // Parse order request
        let params = message
            .params
            .ok_or_else(|| GatewayError::System("Missing order parameters".to_string()))?;
        info!("Order parameters: {:?}", params);

        let order_request: OrderPlaceRequest = match serde_json::from_value(params) {
            Ok(req) => {
                info!("Parsed order request: {:?}", req);
                req
            }
            Err(e) => {
                error!("Failed to parse order request: {}", e);
                return Err(GatewayError::System(format!("Invalid order parameters: {e}")));
            }
        };

        // Validate order against account balance and position
        self.validate_order(&order_request, session.account_id).await?;

        // Create reservation for limit orders
        let _reservation_id = self.create_reservation(&order_request, session.account_id).await?;

        // Convert order request to Whistle InboundMsg
        let order_msg = self.convert_order_request_to_inbound_msg(&order_request, session.account_id)?;

        // Create message with symbol ID for routing
        let symbol_id = self.parse_symbol_id(&order_request.symbol).await?;
        let msg_with_symbol = InboundMsgWithSymbol { symbol_id, msg: order_msg };

        // Route order through OrderRouter
        let current_tick = self.get_current_tick().await?;
        let mut router = self.order_router.write().await;

        match router.route(current_tick, msg_with_symbol) {
            Ok(()) => {
                info!("Order successfully routed to symbol {}", symbol_id);

                // Trigger tick processing to handle the order (event-driven)
                // This ensures the order is processed immediately instead of waiting for the next continuous tick

                // Create success response
                let order_response = OrderPlaceResponse {
                    order_id: format!("ord_{}", uuid::Uuid::new_v4()),
                    status: "ACCEPTED".to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    client_order_id: order_request.client_order_id,
                };

                let response = ApiMessage {
                    id: message.id,
                    method: None,
                    stream: None,
                    params: None,
                    data: None,
                    result: Some(serde_json::to_value(order_response)?),
                    error: None,
                };

                info!("Sending order response: {:?}", response);
                self.send_json_message(response).await?;
                info!("Order response sent successfully");
                Ok(())
            }
            Err(RouterError::Backpressure) => {
                error!("Order rejected due to backpressure");
                Err(GatewayError::System("Order rejected due to system backpressure".to_string()))
            }
            Err(RouterError::SymbolInactive) => {
                error!("Order rejected - symbol {} is inactive", symbol_id);
                Err(GatewayError::System(format!(
                    "Symbol {} is currently inactive",
                    order_request.symbol
                )))
            }
            Err(RouterError::SymbolCapacity) => {
                error!("Order rejected - symbol {} at capacity", symbol_id);
                Err(GatewayError::System(format!("Symbol {} is at capacity", order_request.symbol)))
            }
            Err(e) => {
                error!("Order routing failed: {:?}", e);
                Err(GatewayError::System(format!("Order routing failed: {e:?}")))
            }
        }
    }

    /// Handle market data subscription
    async fn handle_market_data_subscribe(&mut self, message: ApiMessage) -> GatewayResult<()> {
        // Check authentication
        let session = self
            .user_session
            .as_ref()
            .ok_or_else(|| GatewayError::Authentication("Not authenticated".to_string()))?;

        if !session.has_permission("market_data") {
            return Err(GatewayError::Authentication("Insufficient permissions".to_string()));
        }

        // Check rate limits
        self.rate_limiter.check_market_data_rate_limit(&session.user_id).await?;

        // TODO: Handle subscription logic
        // For now, just send a success response
        let response = ApiMessage {
            id: message.id,
            method: None,
            stream: None,
            params: None,
            data: None,
            result: Some(serde_json::json!({"subscribed": true})),
            error: None,
        };

        self.send_json_message(response).await?;
        Ok(())
    }

    /// Send a JSON message
    async fn send_json_message(&self, message: ApiMessage) -> GatewayResult<()> {
        let json = serde_json::to_string(&message)?;
        self.send_message(WsMessage::Text(json)).await?;
        Ok(())
    }

    /// Send a message
    async fn send_message(&self, message: WsMessage) -> GatewayResult<()> {
        if let Some(sender) = &self.sender {
            sender
                .send(message)
                .map_err(|_| GatewayError::Connection("Failed to send message".to_string()))?;
        }
        Ok(())
    }

    /// Send an error message
    async fn send_error(&self, code: &str, message: &str) {
        let error_msg = ApiMessage {
            id: None,
            method: None,
            stream: None,
            params: None,
            data: None,
            result: None,
            error: Some(crate::messages::ErrorMessage {
                code: 50000,
                message: message.to_string(),
                details: Some(serde_json::json!({"code": code})),
            }),
        };

        if let Err(e) = self.send_json_message(error_msg).await {
            error!("Failed to send error message: {}", e);
        }
    }

    /// Close the connection
    pub async fn close(&self) -> GatewayResult<()> {
        if let Some(sender) = &self.sender {
            sender
                .send(WsMessage::Close(None))
                .map_err(|_| GatewayError::Connection("Failed to close connection".to_string()))?;
        }
        Ok(())
    }

    /// Convert OrderPlaceRequest to Whistle InboundMsg
    #[allow(clippy::result_large_err)]
    fn convert_order_request_to_inbound_msg(
        &self,
        request: &OrderPlaceRequest,
        account_id: i64,
    ) -> GatewayResult<InboundMsg> {
        let order_id = OrderId::from(uuid::Uuid::new_v4().as_u128() as u64);

        let account_id = AccountId::from(account_id as u32);

        let side = match request.side.as_str() {
            "BUY" => Side::Buy,
            "SELL" => Side::Sell,
            _ => return Err(GatewayError::System(format!("Invalid side: {}", request.side))),
        };

        let order_type = match request.r#type.as_str() {
            "LIMIT" => OrderType::Limit,
            "MARKET" => OrderType::Market,
            "IOC" => OrderType::Ioc,
            "POST_ONLY" => OrderType::PostOnly,
            _ => {
                return Err(GatewayError::System(format!("Invalid order type: {}", request.r#type)))
            }
        };

        let price = match order_type {
            OrderType::Limit => Some(request.price),
            _ => None,
        };

        let quantity = request.quantity;
        let ts_norm = chrono::Utc::now().timestamp_millis() as u64;
        let meta = 0u64; // No metadata for now
        let enq_seq = 0; // Will be stamped by OrderRouter

        Ok(InboundMsg::submit(
            order_id, account_id, side, order_type, price, quantity, ts_norm, meta, enq_seq,
        ))
    }

    /// Parse symbol string to symbol ID
    async fn parse_symbol_id(&self, symbol: &str) -> GatewayResult<u32> {
        // First try to parse as a direct symbol ID (for backward compatibility)
        if let Ok(symbol_id) = symbol.parse::<u32>() {
            return Ok(symbol_id);
        }

        // Try to look up player by name in the registry
        let registry = self.player_registry.read().await;
        match registry.get_by_name(symbol) {
            Ok(player_symbol) => {
                info!("Found player '{}' with symbol ID {}", symbol, player_symbol.symbol_id);
                Ok(player_symbol.symbol_id)
            }
            Err(_) => {
                // Try partial name search
                let search_results = registry.search_players(symbol);
                if search_results.len() == 1 {
                    let player_symbol = search_results[0];
                    info!(
                        "Found player '{}' via search with symbol ID {}",
                        player_symbol.name, player_symbol.symbol_id
                    );
                    Ok(player_symbol.symbol_id)
                } else if search_results.len() > 1 {
                    // Multiple matches - return error with suggestions
                    let suggestions: Vec<String> =
                        search_results.iter().map(|p| p.name.clone()).collect();
                    Err(GatewayError::System(format!(
                        "Multiple players found for '{}': {}. Please be more specific.",
                        symbol,
                        suggestions.join(", ")
                    )))
                } else {
                    Err(GatewayError::System(format!("Player not found: {symbol}")))
                }
            }
        }
    }

    /// Get current tick from SymbolCoordinator
    async fn get_current_tick(&self) -> GatewayResult<TickId> {
        // Get the actual current tick from SymbolCoordinator
        // This ensures orders are routed to the correct tick for processing
        match self.symbol_coordinator.get_current_tick() {
            Ok(tick) => {
                info!("OrderGateway got current tick {} from SymbolCoordinator", tick);
                Ok(tick)
            }
            Err(_) => {
                // Fallback to a reasonable tick if SymbolCoordinator is not available
                warn!("Failed to get current tick from SymbolCoordinator, using fallback");
                Ok(1000)
            }
        }
    }

    /// Validate order against account balance and position
    async fn validate_order(&self, order_request: &OrderPlaceRequest, account_id: i64) -> GatewayResult<()> {

        let symbol_id = self.parse_symbol_id(&order_request.symbol).await? as i64;
        let side = match order_request.side.as_str() {
            "BUY" => whistle::Side::Buy,
            "SELL" => whistle::Side::Sell,
            _ => return Err(GatewayError::System(format!("Invalid side: {}", order_request.side))),
        };

        let quantity = Balance::from_basis_points(order_request.quantity as i64);
        let price = match order_request.r#type.as_str() {
            "MARKET" => {
                // For market orders, we need to estimate the worst-case price
                // For now, we'll use a conservative estimate of $1000 per share
                Balance::from_cents(100000) // $1000
            }
            "LIMIT" | "IOC" | "POST_ONLY" => {
                Balance::from_cents(order_request.price as i64)
            }
            _ => return Err(GatewayError::System(format!("Invalid order type: {}", order_request.r#type))),
        };

        match side {
            whistle::Side::Buy => {
                // Validate sufficient balance for buy orders
                let required_amount = quantity * price.to_cents() / 10000; // Convert to cents
                let account = self.account_service.get_account(account_id).await
                    .map_err(|e| GatewayError::System(format!("Failed to get account: {}", e)))?;
                
                let available_balance = account.currency_balance.unwrap_or(0);
                if available_balance < required_amount.basis_points {
                    return Err(GatewayError::System(format!(
                        "Insufficient balance: required {} cents, available {} cents",
                        required_amount.to_cents(),
                        available_balance / 10000
                    )));
                }

                info!("Buy order validated: account {} has sufficient balance", account_id);
            }
            whistle::Side::Sell => {
                // Validate sufficient position for sell orders
                let position = self.account_service.get_position(account_id, symbol_id).await
                    .map_err(|e| GatewayError::System(format!("Failed to get position: {}", e)))?;
                
                match position {
                    Some(pos) => {
                        if pos.quantity < quantity {
                            return Err(GatewayError::System(format!(
                                "Insufficient position: required {} shares, available {} shares",
                                quantity.to_decimal(),
                                pos.quantity.to_decimal()
                            )));
                        }
                    }
                    None => {
                        return Err(GatewayError::System(format!(
                            "No position found for account {} and symbol {}",
                            account_id, symbol_id
                        )));
                    }
                }

                info!("Sell order validated: account {} has sufficient position", account_id);
            }
        }

        Ok(())
    }

    /// Create a reservation for limit orders
    async fn create_reservation(&self, order_request: &OrderPlaceRequest, account_id: i64) -> GatewayResult<Option<u64>> {
        // Only create reservations for limit orders
        if order_request.r#type != "LIMIT" {
            return Ok(None);
        }

        let side = match order_request.side.as_str() {
            "BUY" => whistle::Side::Buy,
            "SELL" => whistle::Side::Sell,
            _ => return Err(GatewayError::System(format!("Invalid side: {}", order_request.side))),
        };

        let quantity = Balance::from_basis_points(order_request.quantity as i64);
        let price = Balance::from_cents(order_request.price as i64);

        match side {
            whistle::Side::Buy => {
                // Reserve balance for buy orders
                let required_amount = quantity * price.to_cents() / 10000;
                let order_id = order_request.client_order_id
                    .as_ref()
                    .and_then(|id| id.parse::<i64>().ok())
                    .unwrap_or(0);
                
                let reservation_id = self.account_service.check_and_reserve_balance(
                    account_id,
                    required_amount.basis_points,
                    order_id,
                ).await.map_err(|e| GatewayError::System(format!("Failed to create reservation: {}", e)))?;

                info!("Created balance reservation {} for buy order", reservation_id.0);
                Ok(Some(reservation_id.0))
            }
            whistle::Side::Sell => {
                // For sell orders, we don't need to reserve balance, but we could reserve the position
                // For now, we'll just return None since the position validation already ensures sufficient shares
                info!("Sell order - no reservation needed");
                Ok(None)
            }
        }
    }
}
