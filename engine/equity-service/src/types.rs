//! Type definitions for Equity Valuation Service

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use whistle::TickId;
use crate::error::EquityServiceError;

/// Account equity data with positions and cash balance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountEquityData {
    pub account_id: i64,
    pub cash_balance: i64,  // in cents
    pub positions: HashMap<u32, Position>,  // symbol_id -> position
    pub realized_pnl: i64,  // Total realized P&L in cents
    pub last_updated: DateTime<Utc>,
}

/// Position data for a specific symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol_id: u32,
    pub quantity: i64,      // in basis points
    pub avg_cost: i64,      // in cents
    pub realized_pnl: i64,  // Realized P&L for this position
    pub last_updated: DateTime<Utc>,
}

/// Equity snapshot at a specific point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquitySnapshot {
    pub account_id: i64,
    pub timestamp: DateTime<Utc>,
    pub tick: TickId,
    pub total_equity: i64,      // Total portfolio value in cents
    pub cash_balance: i64,      // Available cash in cents
    pub position_value: i64,    // Value of all positions in cents
    pub unrealized_pnl: i64,    // Unrealized P&L in cents
    pub realized_pnl: i64,      // Realized P&L in cents
    pub day_change: i64,        // $ change today in cents
    pub day_change_percent: f64, // % change today
}

/// Real-time equity update for WebSocket broadcasting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityUpdate {
    pub account_id: i64,
    pub tick: TickId,
    pub timestamp: i64,         // Unix timestamp
    pub total_equity: i64,      // Total portfolio value in cents
    pub cash_balance: i64,      // Available cash in cents
    pub position_value: i64,    // Value of all positions in cents
    pub day_change: i64,        // $ change today in cents
    pub day_change_percent: f64, // % change today
    pub unrealized_pnl: i64,    // Unrealized P&L in cents (paper gains/losses)
    pub realized_pnl: i64,      // Realized P&L in cents (actual trading profits/losses)
}

/// WebSocket message for equity updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityWebSocketMessage {
    pub stream: String,
    pub data: EquityUpdate,
}

/// Equity broadcaster for WebSocket connections
pub struct EquityBroadcaster {
    account_clients: Arc<RwLock<HashMap<i64, Vec<mpsc::UnboundedSender<EquityWebSocketMessage>>>>>,
}

impl EquityBroadcaster {
    /// Create a new equity broadcaster
    pub fn new() -> Self {
        Self {
            account_clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a WebSocket client for an account
    pub async fn add_equity_client(
        &self,
        account_id: i64,
        sender: mpsc::UnboundedSender<EquityWebSocketMessage>,
    ) {
        let mut clients = self.account_clients.write().await;
        clients.entry(account_id).or_insert_with(Vec::new).push(sender);
    }

    /// Remove a WebSocket client for an account
    pub async fn remove_equity_client(
        &self,
        account_id: i64,
        sender: &mpsc::UnboundedSender<EquityWebSocketMessage>,
    ) {
        let mut clients = self.account_clients.write().await;
        if let Some(account_clients) = clients.get_mut(&account_id) {
            account_clients.retain(|s| !s.same_channel(sender));
            if account_clients.is_empty() {
                clients.remove(&account_id);
            }
        }
    }

    /// Send equity update to all clients for an account
    pub async fn send_equity_update(
        &self,
        account_id: i64,
        equity_data: EquityUpdate,
    ) -> Result<(), EquityServiceError> {
        let clients = self.account_clients.read().await;
        if let Some(account_clients) = clients.get(&account_id) {
            let message = EquityWebSocketMessage {
                stream: "equity".to_string(),
                data: equity_data,
            };

            // Send to all clients for this account
            for sender in account_clients {
                if let Err(e) = sender.send(message.clone()) {
                    tracing::warn!("Failed to send equity update to client: {}", e);
                }
            }
        }
        Ok(())
    }

    /// Get number of active clients for an account
    pub async fn get_client_count(&self, account_id: i64) -> usize {
        let clients = self.account_clients.read().await;
        clients.get(&account_id).map_or(0, |v| v.len())
    }

    /// Get total number of active clients
    pub async fn get_total_client_count(&self) -> usize {
        let clients = self.account_clients.read().await;
        clients.values().map(|v| v.len()).sum()
    }
}

impl Default for EquityBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}
