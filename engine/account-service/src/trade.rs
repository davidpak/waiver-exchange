//! Trade representation and settlement

use crate::balance::Balance;
use crate::position::TradeSide;
use serde::{Deserialize, Serialize};

/// Trade represents a completed trade transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: i64,
    pub account_id: i64,
    pub symbol_id: i64,
    pub side: TradeSide,
    pub quantity: Balance, // In basis points (fractional shares)
    pub price: Balance,    // Price in cents
    pub timestamp: chrono::NaiveDateTime,
    pub order_id: i64,
}

impl Trade {
    /// Create a new trade
    pub fn new(
        id: i64,
        account_id: i64,
        symbol_id: i64,
        side: TradeSide,
        quantity: Balance,
        price: Balance,
        order_id: i64,
    ) -> Self {
        Self {
            id,
            account_id,
            symbol_id,
            side,
            quantity,
            price,
            timestamp: chrono::Utc::now().naive_utc(),
            order_id,
        }
    }

    /// Get the total value of the trade
    pub fn total_value(&self) -> Balance {
        self.quantity * self.price.to_cents()
    }

    /// Get the commission (assuming 0% for now)
    pub fn commission(&self) -> Balance {
        Balance::default()
    }

    /// Get the net amount (total value minus commission)
    pub fn net_amount(&self) -> Balance {
        self.total_value() - self.commission()
    }

    /// Get the cash impact of this trade
    pub fn cash_impact(&self) -> Balance {
        match self.side {
            TradeSide::Buy => -self.net_amount(),
            TradeSide::Sell => self.net_amount(),
        }
    }

    /// Get the position impact of this trade
    pub fn position_impact(&self) -> Balance {
        match self.side {
            TradeSide::Buy => self.quantity,
            TradeSide::Sell => -self.quantity,
        }
    }
}

/// Trade details for settlement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeDetails {
    pub account_id: i64,
    pub symbol_id: i64,
    pub side: TradeSide,
    pub quantity: Balance,
    pub price: Balance,
    pub order_id: i64,
}

impl From<&Trade> for TradeDetails {
    fn from(trade: &Trade) -> Self {
        Self {
            account_id: trade.account_id,
            symbol_id: trade.symbol_id,
            side: trade.side,
            quantity: trade.quantity,
            price: trade.price,
            order_id: trade.order_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trade_creation() {
        let trade = Trade::new(
            1,
            100,
            1,
            TradeSide::Buy,
            Balance::from_basis_points(1000000), // 100 shares
            Balance::from_cents(1000),           // $10
            123,
        );

        assert_eq!(trade.account_id, 100);
        assert_eq!(trade.symbol_id, 1);
        assert_eq!(trade.side, TradeSide::Buy);
        assert_eq!(trade.total_value(), Balance::from_cents(100000)); // $1000
    }

    #[test]
    fn test_trade_cash_impact() {
        let buy_trade = Trade::new(
            1,
            100,
            1,
            TradeSide::Buy,
            Balance::from_basis_points(1000000), // 100 shares
            Balance::from_cents(1000),           // $10
            123,
        );

        let sell_trade = Trade::new(
            2,
            100,
            1,
            TradeSide::Sell,
            Balance::from_basis_points(1000000), // 100 shares
            Balance::from_cents(1000),           // $10
            124,
        );

        assert_eq!(buy_trade.cash_impact(), Balance::from_cents(-100000)); // -$1000
        assert_eq!(sell_trade.cash_impact(), Balance::from_cents(100000)); // +$1000
    }

    #[test]
    fn test_trade_position_impact() {
        let buy_trade = Trade::new(
            1,
            100,
            1,
            TradeSide::Buy,
            Balance::from_basis_points(1000000), // 100 shares
            Balance::from_cents(1000),           // $10
            123,
        );

        let sell_trade = Trade::new(
            2,
            100,
            1,
            TradeSide::Sell,
            Balance::from_basis_points(1000000), // 100 shares
            Balance::from_cents(1000),           // $10
            124,
        );

        assert_eq!(buy_trade.position_impact(), Balance::from_basis_points(1000000)); // +100 shares
        assert_eq!(sell_trade.position_impact(), Balance::from_basis_points(-1000000));
        // -100 shares
    }
}
