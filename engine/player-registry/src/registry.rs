use crate::hashing::ConsistentHasher;
use crate::types::{PlayerSymbol, SymbolLookupError};
use player_scraper::types::{Player, PlayerData};
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};

/// Player Registry - Maps NFL players to trading symbols
///
/// This registry loads player data from the scraper and creates
/// consistent symbol mappings for the trading system.
pub struct PlayerRegistry {
    /// Map from symbol ID to PlayerSymbol
    symbols_by_id: HashMap<u32, PlayerSymbol>,

    /// Map from player name to symbol ID (for quick lookup)
    symbols_by_name: HashMap<String, u32>,

    /// Total number of symbols
    symbol_count: u32,
}

impl PlayerRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self { symbols_by_id: HashMap::new(), symbols_by_name: HashMap::new(), symbol_count: 0 }
    }

    /// Load player data from JSON file and create symbol mappings
    pub async fn load_from_file<P: AsRef<Path>>(
        &mut self,
        file_path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Loading player data from: {:?}", file_path.as_ref());

        // Read and parse the JSON file
        let json_content = tokio::fs::read_to_string(&file_path).await?;
        let player_data: PlayerData = serde_json::from_str(&json_content)?;

        info!("Loaded {} players from file", player_data.players.len());

        // Create symbol mappings
        self.create_symbol_mappings(&player_data.players)?;

        info!("Created {} symbol mappings", self.symbol_count);
        Ok(())
    }

    /// Load player data, assign symbol IDs, and save updated JSON file
    pub async fn load_and_assign_symbols<P: AsRef<Path>>(
        &mut self,
        file_path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Loading player data and assigning symbol IDs from: {:?}", file_path.as_ref());

        // Read and parse the JSON file
        let json_content = tokio::fs::read_to_string(&file_path).await?;
        let mut player_data: PlayerData = serde_json::from_str(&json_content)?;

        info!("Loaded {} players from file", player_data.players.len());

        // Assign symbol IDs to players
        let max_symbols = (player_data.players.len() * 2) as u32;
        for player in &mut player_data.players {
            let symbol_id = ConsistentHasher::hash_to_symbol_id(
                &player.name,
                &player.position,
                &player.team,
                max_symbols,
            );
            player.symbol_id = Some(symbol_id);
        }

        // Save updated JSON file with symbol IDs
        let updated_json = serde_json::to_string_pretty(&player_data)?;
        tokio::fs::write(&file_path, updated_json).await?;
        info!("Updated JSON file with symbol IDs");

        // Create symbol mappings
        self.create_symbol_mappings(&player_data.players)?;

        info!("Created {} symbol mappings", self.symbol_count);
        Ok(())
    }

    /// Create symbol mappings from player data
    fn create_symbol_mappings(&mut self, players: &[Player]) -> Result<(), SymbolLookupError> {
        // Clear existing mappings
        self.symbols_by_id.clear();
        self.symbols_by_name.clear();

        // Use a larger range to reduce collisions (2x the number of players)
        let max_symbols = (players.len() * 2) as u32;

        // Create symbol for each player
        for player in players {
            let mut symbol_id = ConsistentHasher::hash_to_symbol_id(
                &player.name,
                &player.position,
                &player.team,
                max_symbols,
            );

            // Handle hash collisions by linear probing
            let mut attempts = 0;
            while self.symbols_by_id.contains_key(&symbol_id) && attempts < 1000 {
                symbol_id = (symbol_id + 1) % max_symbols;
                attempts += 1;
            }

            if attempts >= 1000 {
                warn!("Could not find available symbol ID for player: {}", player.name);
                continue;
            }

            if attempts > 0 {
                info!(
                    "Resolved hash collision for {} after {} attempts, using symbol ID {}",
                    player.name, attempts, symbol_id
                );
            }

            let player_symbol = PlayerSymbol::new(
                symbol_id,
                player.name.clone(),
                player.position.clone(),
                player.team.clone(),
                player.projected_points,
            );

            // Store in both maps
            self.symbols_by_id.insert(symbol_id, player_symbol);
            self.symbols_by_name.insert(player.name.clone(), symbol_id);
        }

        self.symbol_count = self.symbols_by_id.len() as u32;
        Ok(())
    }

    /// Get a player symbol by symbol ID
    pub fn get_by_symbol_id(&self, symbol_id: u32) -> Result<&PlayerSymbol, SymbolLookupError> {
        self.symbols_by_id.get(&symbol_id).ok_or(SymbolLookupError::InvalidSymbolId(symbol_id))
    }

    /// Get a player symbol by player name
    pub fn get_by_name(&self, name: &str) -> Result<&PlayerSymbol, SymbolLookupError> {
        let symbol_id = self
            .symbols_by_name
            .get(name)
            .ok_or_else(|| SymbolLookupError::PlayerNotFound(name.to_string()))?;

        self.get_by_symbol_id(*symbol_id)
    }

    /// Get all player symbols
    pub fn get_all_symbols(&self) -> Vec<&PlayerSymbol> {
        self.symbols_by_id.values().collect()
    }

    /// Get top N players by projected points
    pub fn get_top_players(&self, limit: usize) -> Vec<&PlayerSymbol> {
        let mut symbols: Vec<&PlayerSymbol> = self.symbols_by_id.values().collect();
        symbols.sort_by(|a, b| b.projected_points.partial_cmp(&a.projected_points).unwrap());
        symbols.truncate(limit);
        symbols
    }

    /// Get symbol count
    pub fn symbol_count(&self) -> u32 {
        self.symbol_count
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.symbols_by_id.is_empty()
    }

    /// Search for players by partial name match
    pub fn search_players(&self, query: &str) -> Vec<&PlayerSymbol> {
        let query_lower = query.to_lowercase();
        self.symbols_by_id
            .values()
            .filter(|symbol| symbol.name.to_lowercase().contains(&query_lower))
            .collect()
    }
}

impl Default for PlayerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use player_scraper::types::Player;

    fn create_test_players() -> Vec<Player> {
        vec![
            Player {
                player_id: "2560757".to_string(),
                name: "Lamar Jackson".to_string(),
                position: "QB".to_string(),
                team: "BAL".to_string(),
                projected_points: 351.96,
                symbol_id: None,
                rank: Some(1),
            },
            Player {
                player_id: "2560955".to_string(),
                name: "Josh Allen".to_string(),
                position: "QB".to_string(),
                team: "BUF".to_string(),
                projected_points: 341.48,
                symbol_id: None,
                rank: Some(2),
            },
        ]
    }

    #[test]
    fn test_registry_creation() {
        let mut registry = PlayerRegistry::new();
        let players = create_test_players();

        registry.create_symbol_mappings(&players).unwrap();

        assert_eq!(registry.symbol_count(), 2);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_symbol_lookup() {
        let mut registry = PlayerRegistry::new();
        let players = create_test_players();

        registry.create_symbol_mappings(&players).unwrap();

        // Test lookup by name
        let lamar = registry.get_by_name("Lamar Jackson").unwrap();
        assert_eq!(lamar.name, "Lamar Jackson");
        assert_eq!(lamar.position, "QB");
        assert_eq!(lamar.team, "BAL");

        // Test lookup by symbol ID
        let symbol_id = lamar.symbol_id;
        let lamar_by_id = registry.get_by_symbol_id(symbol_id).unwrap();
        assert_eq!(lamar_by_id.name, "Lamar Jackson");
    }

    #[test]
    fn test_search_players() {
        let mut registry = PlayerRegistry::new();
        let players = create_test_players();

        registry.create_symbol_mappings(&players).unwrap();

        // Test partial name search
        let results = registry.search_players("Lamar");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Lamar Jackson");

        // Test case insensitive search
        let results = registry.search_players("josh");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Josh Allen");
    }
}
