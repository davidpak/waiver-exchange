use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Consistent hashing for player-to-symbol mapping
///
/// This ensures that the same player always maps to the same symbol ID
/// across restarts and different runs.
pub struct ConsistentHasher;

impl ConsistentHasher {
    /// Hash a player name to a symbol ID
    ///
    /// Uses a combination of player name, position, and team to create
    /// a deterministic hash that maps to a symbol ID in the range [0, max_symbols)
    pub fn hash_to_symbol_id(name: &str, position: &str, team: &str, max_symbols: u32) -> u32 {
        // Create a composite key for consistent hashing
        let composite_key = format!("{name}|{position}|{team}");

        // Hash the composite key
        let mut hasher = DefaultHasher::new();
        composite_key.hash(&mut hasher);
        let hash = hasher.finish();

        // Map to symbol ID range
        (hash % max_symbols as u64) as u32
    }

    /// Hash a player name only (for lookup by name)
    ///
    /// This is used when we only have the player name and need to find
    /// the corresponding symbol. We'll need to search through the registry
    /// since we can't reverse the hash.
    pub fn hash_player_name(name: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consistent_hashing() {
        // Same input should always produce same output
        let id1 = ConsistentHasher::hash_to_symbol_id("Lamar Jackson", "QB", "BAL", 500);
        let id2 = ConsistentHasher::hash_to_symbol_id("Lamar Jackson", "QB", "BAL", 500);
        assert_eq!(id1, id2);

        // Different players should (likely) produce different IDs
        let id3 = ConsistentHasher::hash_to_symbol_id("Josh Allen", "QB", "BUF", 500);
        assert_ne!(id1, id3);

        // Symbol IDs should be in valid range
        assert!(id1 < 500);
        assert!(id3 < 500);
    }

    #[test]
    fn test_symbol_id_range() {
        // Test that all generated IDs are in valid range
        for i in 0..1000 {
            let name = format!("Player{i}");
            let id = ConsistentHasher::hash_to_symbol_id(&name, "QB", "TEAM", 500);
            assert!(id < 500, "Symbol ID {id} is out of range for player {name}");
        }
    }
}
