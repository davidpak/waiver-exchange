//! Balance reservation system for limit orders

use crate::balance::Balance;
use serde::{Deserialize, Serialize};

/// Reservation ID wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReservationId(pub u64);

/// Reservation status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReservationStatus {
    Active,
    Settled,
    Expired,
    Cancelled,
}

/// Reservation represents a temporary hold on account balance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reservation {
    pub id: ReservationId,
    pub account_id: i64,
    pub amount: Balance, // Amount in cents
    pub order_id: i64,
    pub status: ReservationStatus,
    pub created_at: chrono::NaiveDateTime,
    pub expires_at: chrono::NaiveDateTime,
}

impl Reservation {
    /// Create a new reservation
    pub fn new(
        id: ReservationId,
        account_id: i64,
        amount: Balance,
        order_id: i64,
        expires_at: chrono::NaiveDateTime,
    ) -> Self {
        Self {
            id,
            account_id,
            amount,
            order_id,
            status: ReservationStatus::Active,
            created_at: chrono::Utc::now().naive_utc(),
            expires_at,
        }
    }
    
    /// Check if reservation is expired
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().naive_utc() > self.expires_at
    }
    
    /// Check if reservation is active
    pub fn is_active(&self) -> bool {
        self.status == ReservationStatus::Active && !self.is_expired()
    }
    
    /// Mark reservation as settled
    pub fn settle(&mut self) {
        self.status = ReservationStatus::Settled;
    }
    
    /// Mark reservation as cancelled
    pub fn cancel(&mut self) {
        self.status = ReservationStatus::Cancelled;
    }
    
    /// Mark reservation as expired
    pub fn expire(&mut self) {
        self.status = ReservationStatus::Expired;
    }
}

/// Reservation manager for tracking active reservations
#[derive(Debug, Default)]
pub struct ReservationManager {
    reservations: std::collections::HashMap<ReservationId, Reservation>,
}

impl ReservationManager {
    /// Create a new reservation manager
    pub fn new() -> Self {
        Self {
            reservations: std::collections::HashMap::new(),
        }
    }
    
    /// Add a reservation
    pub fn add_reservation(&mut self, reservation: Reservation) {
        self.reservations.insert(reservation.id, reservation);
    }
    
    /// Get a reservation by ID
    pub fn get_reservation(&self, id: ReservationId) -> Option<&Reservation> {
        self.reservations.get(&id)
    }
    
    /// Get a reservation by ID (mutable)
    pub fn get_reservation_mut(&mut self, id: ReservationId) -> Option<&mut Reservation> {
        self.reservations.get_mut(&id)
    }
    
    /// Remove a reservation
    pub fn remove_reservation(&mut self, id: ReservationId) -> Option<Reservation> {
        self.reservations.remove(&id)
    }
    
    /// Get all active reservations for an account
    pub fn get_active_reservations(&self, account_id: i64) -> Vec<&Reservation> {
        self.reservations
            .values()
            .filter(|r| r.account_id == account_id && r.is_active())
            .collect()
    }
    
    /// Get total reserved amount for an account
    pub fn get_total_reserved(&self, account_id: i64) -> Balance {
        self.get_active_reservations(account_id)
            .iter()
            .map(|r| r.amount)
            .fold(Balance::default(), |acc, amount| acc + amount)
    }
    
    /// Clean up expired reservations
    pub fn cleanup_expired(&mut self) -> Vec<ReservationId> {
        let mut expired_ids = Vec::new();
        
        for (id, reservation) in self.reservations.iter_mut() {
            if reservation.is_expired() && reservation.status == ReservationStatus::Active {
                reservation.expire();
                expired_ids.push(*id);
            }
        }
        
        expired_ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    
    #[test]
    fn test_reservation_creation() {
        let expires_at = (Utc::now() + chrono::Duration::days(7)).naive_utc();
        let reservation = Reservation::new(
            ReservationId(1),
            100,
            Balance::from_cents(1000), // $10
            123,
            expires_at,
        );
        
        assert_eq!(reservation.id, ReservationId(1));
        assert_eq!(reservation.account_id, 100);
        assert_eq!(reservation.amount, Balance::from_cents(1000));
        assert_eq!(reservation.status, ReservationStatus::Active);
        assert!(reservation.is_active());
    }
    
    #[test]
    fn test_reservation_expiry() {
        let expires_at = (Utc::now() - chrono::Duration::days(1)).naive_utc(); // Expired
        let reservation = Reservation::new(
            ReservationId(1),
            100,
            Balance::from_cents(1000),
            123,
            expires_at,
        );
        
        assert!(reservation.is_expired());
        assert!(!reservation.is_active());
    }
    
    #[test]
    fn test_reservation_manager() {
        let mut manager = ReservationManager::new();
        
        let reservation = Reservation::new(
            ReservationId(1),
            100,
            Balance::from_cents(1000),
            123,
            (Utc::now() + chrono::Duration::days(7)).naive_utc(),
        );
        
        manager.add_reservation(reservation);
        
        assert_eq!(manager.get_total_reserved(100), Balance::from_cents(1000));
        assert_eq!(manager.get_total_reserved(200), Balance::default());
        
        let active_reservations = manager.get_active_reservations(100);
        assert_eq!(active_reservations.len(), 1);
    }
}
