//! Market data broadcasting for the OrderGateway

use crate::error::GatewayError;
use crate::messages::MarketDataUpdate;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message;

/// Market data broadcaster that sends updates to connected clients
pub struct MarketDataBroadcaster {
    /// Connected clients (user_id -> WebSocket sender)
    clients: Arc<RwLock<HashMap<String, tokio::sync::mpsc::UnboundedSender<Message>>>>,

    /// Market data cache (symbol -> latest data)
    market_data_cache: Arc<RwLock<HashMap<String, MarketDataUpdate>>>,
}

impl Default for MarketDataBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl MarketDataBroadcaster {
    /// Create a new market data broadcaster
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            market_data_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a client to receive market data updates
    pub async fn add_client(
        &self,
        user_id: String,
        sender: tokio::sync::mpsc::UnboundedSender<Message>,
    ) {
        let mut clients = self.clients.write().await;
        clients.insert(user_id, sender);
    }

    /// Remove a client
    pub async fn remove_client(&self, user_id: &str) {
        let mut clients = self.clients.write().await;
        clients.remove(user_id);
    }

    /// Update market data for a symbol
    pub async fn update_market_data(&self, symbol: String, data: MarketDataUpdate) {
        let mut cache = self.market_data_cache.write().await;
        cache.insert(symbol.clone(), data.clone());
    }

    /// Broadcast market data updates to all connected clients
    pub async fn broadcast_updates(&self) -> Result<(), GatewayError> {
        let cache = self.market_data_cache.read().await;
        let clients = self.clients.read().await;

        if cache.is_empty() || clients.is_empty() {
            return Ok(());
        }

        // Create market data message
        let market_data_msg = serde_json::json!({
            "stream": "market_data",
            "data": cache.values().collect::<Vec<_>>()
        });

        let message = Message::Text(serde_json::to_string(&market_data_msg)?);

        // Send to all clients
        let mut failed_clients = Vec::new();
        for (user_id, sender) in clients.iter() {
            if sender.send(message.clone()).is_err() {
                failed_clients.push(user_id.clone());
            }
        }

        // Remove failed clients
        if !failed_clients.is_empty() {
            let mut clients = self.clients.write().await;
            for user_id in failed_clients {
                clients.remove(&user_id);
            }
        }

        Ok(())
    }

    /// Get current market data for a symbol
    pub async fn get_market_data(&self, symbol: &str) -> Option<MarketDataUpdate> {
        let cache = self.market_data_cache.read().await;
        cache.get(symbol).cloned()
    }

    /// Get all market data
    pub async fn get_all_market_data(&self) -> HashMap<String, MarketDataUpdate> {
        let cache = self.market_data_cache.read().await;
        cache.clone()
    }

    /// Get the number of connected clients
    pub async fn client_count(&self) -> usize {
        let clients = self.clients.read().await;
        clients.len()
    }
}
