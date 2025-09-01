use siphasher::sip::SipHasher13;
use std::hash::{Hash, Hasher};

/// Shard identifier
pub type ShardId = u32;

/// Symbol-to-shard mapping
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymbolShard {
    pub symbol_id: u32,
    pub shard_id: ShardId,
}

/// Deterministic symbol sharding using modulo
/// In production, we could use SipHash for symbol_id generation from PlayerUID
pub fn shard_for_symbol(symbol_id: u32, num_shards: u32) -> ShardId {
    symbol_id % num_shards
}

/// Generate symbol ID from player identifier (placeholder)
/// In practice, this would use SipHash64 over canonical PlayerUID
pub fn symbol_id_from_player(player_uid: &str) -> u32 {
    let mut hasher = SipHasher13::new_with_keys(0xDEADBEEF, 0xCAFEBABE);
    player_uid.hash(&mut hasher);
    hasher.finish() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_sharding() {
        let num_shards = 4;

        // Test that same symbol always maps to same shard
        assert_eq!(shard_for_symbol(1, num_shards), 1);
        assert_eq!(shard_for_symbol(1, num_shards), 1);

        // Test modulo distribution
        assert_eq!(shard_for_symbol(0, num_shards), 0);
        assert_eq!(shard_for_symbol(1, num_shards), 1);
        assert_eq!(shard_for_symbol(4, num_shards), 0);
        assert_eq!(shard_for_symbol(5, num_shards), 1);
    }

    #[test]
    fn test_symbol_id_generation() {
        let player1 = "Ja'Marr Chase";
        let player2 = "Derrick Henry";

        let id1 = symbol_id_from_player(player1);
        let id2 = symbol_id_from_player(player2);

        // Should be deterministic
        assert_eq!(id1, symbol_id_from_player(player1));
        assert_eq!(id2, symbol_id_from_player(player2));

        // Should be different
        assert_ne!(id1, id2);
    }
}
