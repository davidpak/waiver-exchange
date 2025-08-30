use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub name: String,
    pub accounts: u32,
    pub created: u64,
    pub last_activity: u64,
    pub participants: HashMap<u32, ParticipantInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantInfo {
    pub name: String,
    pub account_type: AccountType,
    pub connected: bool,
    pub last_seen: u64,
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
