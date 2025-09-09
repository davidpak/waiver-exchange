// Execution ID allocation for ExecutionManager

use crate::config::ExecutionIdConfig;
use std::sync::atomic::{AtomicU64, Ordering};
// Removed unused time imports

/// Execution ID allocator for generating globally unique execution IDs
pub struct ExecutionIdAllocator {
    config: ExecutionIdConfig,
    counter: AtomicU64,
    shard_mask: u64,
}

impl ExecutionIdAllocator {
    /// Create a new execution ID allocator
    pub fn new(config: ExecutionIdConfig) -> Self {
        let shard_mask = match config.mode {
            crate::config::ExecutionIdMode::Monotonic => 0,
            crate::config::ExecutionIdMode::Sharded => {
                if let Some(shard_id) = config.shard_id {
                    (shard_id as u64) << 48 // Use upper 16 bits for shard ID
                } else {
                    0
                }
            }
        };

        Self { config, counter: AtomicU64::new(0), shard_mask }
    }

    /// Allocate a new execution ID
    ///
    /// For monotonic mode: returns a simple incrementing counter
    /// For sharded mode: returns (shard_id << 48) | counter
    pub fn allocate(&self) -> ExecutionId {
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        self.shard_mask | counter
    }

    /// Allocate an execution ID with tick information
    ///
    /// This is used when we want to embed tick information in the execution ID
    /// for deterministic replay and debugging purposes.
    pub fn allocate_with_tick(&self, tick: u64) -> ExecutionId {
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        let tick_shifted = tick << self.config.tick_shift_bits;
        self.shard_mask | tick_shifted | (counter & ((1 << self.config.tick_shift_bits) - 1))
    }

    /// Get the current counter value (for testing and monitoring)
    pub fn current_counter(&self) -> u64 {
        self.counter.load(Ordering::Relaxed)
    }

    /// Get the shard mask (for testing and monitoring)
    pub fn shard_mask(&self) -> u64 {
        self.shard_mask
    }
}

/// Execution ID type alias for clarity
pub type ExecutionId = u64;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ExecutionIdConfig, ExecutionIdMode};

    #[test]
    fn test_monotonic_allocation() {
        let config = ExecutionIdConfig {
            mode: ExecutionIdMode::Monotonic,
            shard_id: None,
            tick_shift_bits: 12,
        };

        let allocator = ExecutionIdAllocator::new(config);

        let id1 = allocator.allocate();
        let id2 = allocator.allocate();
        let id3 = allocator.allocate();

        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(id3, 2);
    }

    #[test]
    fn test_sharded_allocation() {
        let config = ExecutionIdConfig {
            mode: ExecutionIdMode::Sharded,
            shard_id: Some(1),
            tick_shift_bits: 12,
        };

        let allocator = ExecutionIdAllocator::new(config);

        let id1 = allocator.allocate();
        let id2 = allocator.allocate();

        // Check that shard ID is embedded in upper bits
        assert_eq!(id1 & 0xFFFF000000000000, 0x0001000000000000);
        assert_eq!(id2 & 0xFFFF000000000000, 0x0001000000000000);

        // Check that counter is in lower bits
        assert_eq!(id1 & 0x0000FFFFFFFFFFFF, 0);
        assert_eq!(id2 & 0x0000FFFFFFFFFFFF, 1);
    }

    #[test]
    fn test_tick_based_allocation() {
        let config = ExecutionIdConfig {
            mode: ExecutionIdMode::Monotonic,
            shard_id: None,
            tick_shift_bits: 12,
        };

        let allocator = ExecutionIdAllocator::new(config);

        let id1 = allocator.allocate_with_tick(100);
        let id2 = allocator.allocate_with_tick(100);
        let id3 = allocator.allocate_with_tick(101);

        // Check that tick is embedded correctly
        assert_eq!(id1 >> 12, 100);
        assert_eq!(id2 >> 12, 100);
        assert_eq!(id3 >> 12, 101);

        // Check that counter is in lower bits
        assert_eq!(id1 & 0xFFF, 0);
        assert_eq!(id2 & 0xFFF, 1);
        assert_eq!(id3 & 0xFFF, 2);
    }

    #[test]
    fn test_concurrent_allocation() {
        use std::sync::Arc;
        use std::thread;

        let config = ExecutionIdConfig {
            mode: ExecutionIdMode::Monotonic,
            shard_id: None,
            tick_shift_bits: 12,
        };

        let allocator = Arc::new(ExecutionIdAllocator::new(config));
        let mut handles = vec![];

        // Spawn multiple threads to allocate IDs concurrently
        for _ in 0..4 {
            let allocator = allocator.clone();
            let handle = thread::spawn(move || {
                let mut ids = vec![];
                for _ in 0..100 {
                    ids.push(allocator.allocate());
                }
                ids
            });
            handles.push(handle);
        }

        // Collect all allocated IDs
        let mut all_ids = vec![];
        for handle in handles {
            all_ids.extend(handle.join().unwrap());
        }

        // Sort and verify uniqueness
        all_ids.sort();
        for (i, &id) in all_ids.iter().enumerate() {
            assert_eq!(id, i as u64);
        }

        // Verify we got the expected number of IDs
        assert_eq!(all_ids.len(), 400);
    }
}
