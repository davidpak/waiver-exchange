//! AccountService - User account, balance, and risk management
//!
//! This crate provides the AccountService which manages user accounts, balances,
//! positions, and risk validation in the waiver-exchange system.

pub mod account;
pub mod balance;
pub mod config;
pub mod error;
pub mod oauth;
pub mod position;
pub mod reservation;
pub mod sleeper;
pub mod trade;

pub use account::AccountService;
pub use config::AccountServiceConfig;
pub use error::AccountServiceError;

// Re-export commonly used types
pub use balance::Balance;
pub use position::Position;
pub use reservation::{Reservation, ReservationId};
pub use trade::Trade;

// Result type alias
pub type Result<T> = std::result::Result<T, AccountServiceError>;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_balance_creation() {
        let balance = Balance::from_cents(100);
        assert_eq!(balance.to_cents(), 100);
    }
    
    #[test]
    fn test_reservation_id() {
        let id1 = ReservationId(123);
        let id2 = ReservationId(123);
        assert_eq!(id1, id2);
    }
}