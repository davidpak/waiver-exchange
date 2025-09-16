//! Balance type for financial calculations with fractional share support

use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::{Deserialize, Serialize};
use std::ops::{Add, Sub, Mul, Div, Neg};

/// Balance represents a monetary amount in cents with high precision
/// 
/// Uses basis points (1/10000th) for fractional share calculations
/// Example: $1.00 = 100 cents, 0.5 shares = 5000 basis points
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Balance {
    /// Amount in basis points (1/10000th of a unit)
    pub basis_points: i64,
}

impl Balance {
    /// Create a balance from cents
    pub fn from_cents(cents: i64) -> Self {
        Self {
            basis_points: cents * 10000, // Convert cents to basis points
        }
    }
    
    /// Create a balance from basis points
    pub fn from_basis_points(basis_points: i64) -> Self {
        Self { basis_points }
    }
    
    /// Get the value in cents
    pub fn to_cents(self) -> i64 {
        self.basis_points / 10000
    }
    
    /// Get the value as a decimal
    pub fn to_decimal(self) -> Decimal {
        Decimal::from(self.basis_points) / Decimal::from(10000)
    }
    
    /// Create from decimal
    pub fn from_decimal(decimal: Decimal) -> Self {
        let basis_points = (decimal * Decimal::from(10000)).to_i64().unwrap_or(0);
        Self { basis_points }
    }
    
    /// Check if balance is zero
    pub fn is_zero(self) -> bool {
        self.basis_points == 0
    }
    
    /// Check if balance is positive
    pub fn is_positive(self) -> bool {
        self.basis_points > 0
    }
    
    /// Check if balance is negative
    pub fn is_negative(self) -> bool {
        self.basis_points < 0
    }
    
    /// Get absolute value
    pub fn abs(self) -> Self {
        Self {
            basis_points: self.basis_points.abs(),
        }
    }
    
    /// Safe subtraction that returns zero if result would be negative
    pub fn safe_sub(self, other: Self) -> Self {
        let result = self.basis_points - other.basis_points;
        Self {
            basis_points: result.max(0),
        }
    }
}

impl Add for Balance {
    type Output = Self;
    
    fn add(self, other: Self) -> Self {
        Self {
            basis_points: self.basis_points + other.basis_points,
        }
    }
}

impl Sub for Balance {
    type Output = Self;
    
    fn sub(self, other: Self) -> Self {
        Self {
            basis_points: self.basis_points - other.basis_points,
        }
    }
}

impl Mul<i64> for Balance {
    type Output = Self;
    
    fn mul(self, rhs: i64) -> Self {
        Self {
            basis_points: self.basis_points * rhs,
        }
    }
}

impl Div<i64> for Balance {
    type Output = Self;
    
    fn div(self, rhs: i64) -> Self {
        Self {
            basis_points: self.basis_points / rhs,
        }
    }
}

impl Neg for Balance {
    type Output = Self;
    
    fn neg(self) -> Self {
        Self {
            basis_points: -self.basis_points,
        }
    }
}

impl Default for Balance {
    fn default() -> Self {
        Self { basis_points: 0 }
    }
}

impl std::fmt::Display for Balance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dollars = self.to_decimal();
        write!(f, "${:.4}", dollars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_balance_creation() {
        let balance = Balance::from_cents(100);
        assert_eq!(balance.basis_points, 1000000);
        assert_eq!(balance.to_cents(), 100);
    }
    
    #[test]
    fn test_balance_arithmetic() {
        let a = Balance::from_cents(100);
        let b = Balance::from_cents(50);
        
        assert_eq!(a + b, Balance::from_cents(150));
        assert_eq!(a - b, Balance::from_cents(50));
        assert_eq!(a * 2, Balance::from_cents(200));
        assert_eq!(a / 2, Balance::from_cents(50));
    }
    
    #[test]
    fn test_fractional_shares() {
        // 0.5 shares at $100 per share = $50
        let shares = Balance::from_basis_points(5000); // 0.5 shares
        let price_cents = 10000; // $100 in cents
        let total = shares * price_cents / 10000; // Convert to cents equivalent
        
        // The result should be 5000 basis points (representing $0.50)
        assert_eq!(total.basis_points, 5000);
        // When converted to cents, it should be 0 (since 5000 basis points = 0.5 cents)
        assert_eq!(total.to_cents(), 0);
    }
    
    #[test]
    fn test_safe_subtraction() {
        let a = Balance::from_cents(50);
        let b = Balance::from_cents(100);
        
        assert_eq!(a.safe_sub(b), Balance::from_cents(0));
        assert_eq!(b.safe_sub(a), Balance::from_cents(50));
    }
}
