use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub name: String,
    pub accounts: u32,
    pub symbols: u32,
    pub created: u64,
    pub last_activity: u64,
    pub participants: HashMap<u32, ParticipantInfo>,
    pub symbols_info: HashMap<u32, SymbolInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantInfo {
    pub name: String,
    pub account_type: AccountType,
    pub connected: bool,
    pub last_seen: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub symbol_id: u32,
    pub name: String,
    pub position: String,
    pub active: bool,
    pub last_trade_price: Option<u32>,
    pub last_trade_qty: Option<u32>,
    pub volume_24h: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountType {
    MarketMaker,
    Trader,
    Institutional,
}

impl AccountType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "marketmaker" | "market_maker" => Some(AccountType::MarketMaker),
            "trader" => Some(AccountType::Trader),
            "institutional" => Some(AccountType::Institutional),
            _ => None,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            AccountType::MarketMaker => "MarketMaker".to_string(),
            AccountType::Trader => "Trader".to_string(),
            AccountType::Institutional => "Institutional".to_string(),
        }
    }
}

impl SymbolInfo {
    pub fn new(symbol_id: u32, name: &str, position: &str) -> Self {
        Self {
            symbol_id,
            name: name.to_string(),
            position: position.to_string(),
            active: false,
            last_trade_price: None,
            last_trade_qty: None,
            volume_24h: 0,
        }
    }
}
