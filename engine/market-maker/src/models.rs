use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use bigdecimal::BigDecimal;

/// Fair price data for a player
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FairPrice {
    pub player_id: i32,
    pub symbol_id: u32,
    pub fair_cents: i64,
    pub source: String,
    pub confidence_score: BigDecimal,
    pub updated_at: DateTime<Utc>,
}

/// Order book state for a symbol
#[derive(Debug, Clone)]
pub struct OrderBookState {
    pub symbol_id: u32,
    pub buy_orders: std::collections::HashMap<u64, u64>, // price -> quantity
    pub sell_orders: std::collections::HashMap<u64, u64>, // price -> quantity
    pub last_trade_price: Option<u64>,
    pub last_trade_quantity: Option<u64>,
    pub last_trade_timestamp: Option<DateTime<Utc>>,
}

/// Market maker quote
#[derive(Debug, Clone)]
pub struct MarketMakerQuote {
    pub symbol_id: u32,
    pub side: QuoteSide,
    pub price_cents: i64,
    pub quantity_bp: i64,
    pub fair_price_cents: i64,
    pub spread_bps: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QuoteSide {
    Bid,  // Buy order
    Ask,  // Sell order
}

/// Market maker decision
#[derive(Debug, Clone)]
pub enum MarketMakerDecision {
    /// Post both bid and ask quotes
    PostBoth { bid: MarketMakerQuote, ask: MarketMakerQuote },
    /// Post only bid quote
    PostBid { bid: MarketMakerQuote },
    /// Post only ask quote
    PostAsk { ask: MarketMakerQuote },
    /// Do nothing
    DoNothing,
}

/// Market maker metrics
#[derive(Debug, Clone, Default)]
pub struct MarketMakerMetrics {
    pub quotes_posted: u64,
    pub quotes_cancelled: u64,
    pub fills_received: u64,
    pub fair_price_updates: u64,
    pub order_book_checks: u64,
    pub errors: u64,
    pub last_update: Option<DateTime<Utc>>,
}

impl OrderBookState {
    /// Check if order book is empty
    pub fn is_empty(&self) -> bool {
        self.buy_orders.is_empty() && self.sell_orders.is_empty()
    }
    
    /// Check if order book is one-sided (only bids or only asks)
    pub fn is_one_sided(&self) -> bool {
        self.buy_orders.is_empty() || self.sell_orders.is_empty()
    }
    
    /// Calculate current spread in basis points
    pub fn spread_bps(&self) -> Option<u32> {
        let best_bid = self.buy_orders.keys().max()?;
        let best_ask = self.sell_orders.keys().min()?;
        
        if best_ask <= best_bid {
            return None; // Invalid spread
        }
        
        let spread_cents = best_ask - best_bid;
        let spread_bps = (spread_cents * 10000) / (*best_bid as u64);
        
        Some(spread_bps as u32)
    }
    
    /// Get best bid price
    pub fn best_bid(&self) -> Option<u64> {
        self.buy_orders.keys().max().copied()
    }
    
    /// Get best ask price
    pub fn best_ask(&self) -> Option<u64> {
        self.sell_orders.keys().min().copied()
    }
}

impl MarketMakerQuote {
    /// Create a new market maker quote
    pub fn new(
        symbol_id: u32,
        side: QuoteSide,
        fair_price_cents: i64,
        spread_bps: u32,
        quantity_bp: i64,
    ) -> Self {
        let price_cents = match side {
            QuoteSide::Bid => fair_price_cents - (fair_price_cents * spread_bps as i64 / 10000),
            QuoteSide::Ask => fair_price_cents + (fair_price_cents * spread_bps as i64 / 10000),
        };
        
        Self {
            symbol_id,
            side,
            price_cents,
            quantity_bp,
            fair_price_cents,
            spread_bps,
        }
    }
}
