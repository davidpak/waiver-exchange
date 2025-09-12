use serde::{Deserialize, Serialize};
use std::fmt;

/// A player symbol represents a fantasy football player as a tradeable asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSymbol {
    /// Unique symbol ID (0-499 for our 500 players)
    pub symbol_id: u32,

    /// Player name (e.g., "Lamar Jackson")
    pub name: String,

    /// Position (e.g., "QB", "RB", "WR", "TE")
    pub position: String,

    /// Team abbreviation (e.g., "BAL", "BUF")
    pub team: String,

    /// 2025 season projected fantasy points
    pub projected_points: f64,

    /// Symbol name for trading (e.g., "LAMAR_JACKSON_QB_BAL")
    pub symbol_name: String,
}

impl PlayerSymbol {
    /// Create a new player symbol
    pub fn new(
        symbol_id: u32,
        name: String,
        position: String,
        team: String,
        projected_points: f64,
    ) -> Self {
        let symbol_name =
            format!("{}_{}_{}", name.to_uppercase().replace(" ", "_"), position, team);

        Self { symbol_id, name, position, team, projected_points, symbol_name }
    }

    /// Get the projected price (same as projected points, no multiplier)
    pub fn projected_price(&self) -> f64 {
        self.projected_points
    }
}

/// Errors that can occur during symbol lookup
#[derive(Debug, Clone)]
pub enum SymbolLookupError {
    /// Player not found in registry
    PlayerNotFound(String),

    /// Invalid symbol ID
    InvalidSymbolId(u32),

    /// Registry not initialized
    RegistryNotInitialized,
}

impl fmt::Display for SymbolLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SymbolLookupError::PlayerNotFound(name) => {
                write!(f, "Player '{name}' not found in registry")
            }
            SymbolLookupError::InvalidSymbolId(id) => {
                write!(f, "Invalid symbol ID: {id}")
            }
            SymbolLookupError::RegistryNotInitialized => {
                write!(f, "Player registry not initialized")
            }
        }
    }
}

impl std::error::Error for SymbolLookupError {}
