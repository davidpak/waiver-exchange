//! Position tracking for user holdings per symbol

use crate::balance::Balance;
use serde::{Deserialize, Serialize};

/// Trade side enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

/// Position represents a user's holdings in a specific symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub account_id: i64,
    pub symbol_id: i64,
    pub quantity: Balance, // In basis points (fractional shares)
    pub avg_cost: Balance, // Average cost in cents
    pub last_updated: chrono::NaiveDateTime,
}

impl Position {
    /// Create a new position
    pub fn new(account_id: i64, symbol_id: i64, quantity: Balance, avg_cost: Balance) -> Self {
        Self {
            account_id,
            symbol_id,
            quantity,
            avg_cost,
            last_updated: chrono::Utc::now().naive_utc(),
        }
    }

    /// Update position with a new trade
    pub fn update_with_trade(&mut self, side: TradeSide, quantity: Balance, price: Balance) {
        match side {
            TradeSide::Buy => {
                // Calculate new average cost
                let existing_cost = self.quantity.to_cents() * self.avg_cost.to_cents();
                let new_cost = quantity.to_cents() * price.to_cents();
                let total_quantity = self.quantity + quantity;

                if total_quantity.is_zero() {
                    self.avg_cost = Balance::default();
                } else {
                    let total_cost = existing_cost + new_cost;
                    let avg_cost_cents = total_cost / total_quantity.to_cents();
                    self.avg_cost = Balance::from_cents(avg_cost_cents);
                }

                self.quantity = total_quantity;
            }
            TradeSide::Sell => {
                // Calculate realized P&L
                let _realized_pnl =
                    (price.to_cents() - self.avg_cost.to_cents()) * quantity.to_cents();

                // Update quantity
                self.quantity = self.quantity.safe_sub(quantity);

                // If position is closed, reset average cost
                if self.quantity.is_zero() {
                    self.avg_cost = Balance::default();
                }
            }
        }

        self.last_updated = chrono::Utc::now().naive_utc();
    }

    /// Get the current market value of the position
    pub fn market_value(&self, current_price: Balance) -> Balance {
        self.quantity * current_price.to_cents()
    }

    /// Get the unrealized P&L
    pub fn unrealized_pnl(&self, current_price: Balance) -> Balance {
        let current_value = self.market_value(current_price);
        let cost_basis = Balance::from_cents(self.quantity.to_cents() * self.avg_cost.to_cents());
        current_value - cost_basis
    }

    /// Check if position is empty
    pub fn is_empty(&self) -> bool {
        self.quantity.is_zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_buy() {
        let mut position = Position::new(1, 1, Balance::default(), Balance::default());

        // Buy 100 shares at $10
        position.update_with_trade(
            TradeSide::Buy,
            Balance::from_basis_points(1000000), // 100 shares
            Balance::from_cents(1000),           // $10
        );

        assert_eq!(position.quantity, Balance::from_basis_points(1000000));
        assert_eq!(position.avg_cost, Balance::from_cents(1000));
    }

    #[test]
    fn test_position_average_cost() {
        let mut position = Position::new(1, 1, Balance::default(), Balance::default());

        // Buy 100 shares at $10
        position.update_with_trade(
            TradeSide::Buy,
            Balance::from_basis_points(1000000), // 100 shares
            Balance::from_cents(1000),           // $10
        );

        // Buy 100 more shares at $20
        position.update_with_trade(
            TradeSide::Buy,
            Balance::from_basis_points(1000000), // 100 shares
            Balance::from_cents(2000),           // $20
        );

        // Average cost should be $15
        assert_eq!(position.quantity, Balance::from_basis_points(2000000));
        assert_eq!(position.avg_cost, Balance::from_cents(1500));
    }

    #[test]
    fn test_position_sell() {
        let mut position =
            Position::new(1, 1, Balance::from_basis_points(1000000), Balance::from_cents(1000));

        // Sell 50 shares at $15
        position.update_with_trade(
            TradeSide::Sell,
            Balance::from_basis_points(500000), // 50 shares
            Balance::from_cents(1500),          // $15
        );

        assert_eq!(position.quantity, Balance::from_basis_points(500000));
        assert_eq!(position.avg_cost, Balance::from_cents(1000)); // Unchanged
    }

    #[test]
    fn test_position_close() {
        let mut position =
            Position::new(1, 1, Balance::from_basis_points(1000000), Balance::from_cents(1000));

        // Sell all shares
        position.update_with_trade(
            TradeSide::Sell,
            Balance::from_basis_points(1000000), // 100 shares
            Balance::from_cents(1500),           // $15
        );

        assert!(position.is_empty());
        assert_eq!(position.avg_cost, Balance::default());
    }
}
