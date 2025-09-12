//! Player Registry - Maps NFL players to trading symbols
//!
//! This module provides consistent hashing to map fantasy football players
//! to unique symbol IDs for the trading system.

pub mod hashing;
pub mod registry;
pub mod types;

pub use registry::PlayerRegistry;
pub use types::{PlayerSymbol, SymbolLookupError};
