// SymbolCoordinator - per-symbol engine lifecycle management
#![allow(dead_code)]

mod coordinator;
mod placement;
mod queue;
mod registry;
mod types;

#[cfg(test)]
mod integration_test;

pub use coordinator::SymbolCoordinator;
pub use types::{CoordError, CoordinatorConfig, ReadyAtTick};

// Define the trait locally for OrderRouter compatibility
pub trait SymbolCoordinatorApi {
    fn ensure_active(&self, symbol_id: u32) -> Result<ReadyAtTick, CoordError>;
    fn release_if_idle(&self, symbol_id: u32);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placement::{EngineThreadPool, HashBasedPolicy, PlacementPolicy, RoundRobinPolicy};
    use crate::queue::QueueAllocator;
    use crate::registry::SymbolRegistry;

    #[test]
    fn test_coordinator_creation() {
        let config = CoordinatorConfig::default();
        let coordinator = SymbolCoordinator::new(config);

        assert_eq!(coordinator.current_tick(), 0);
        assert_eq!(coordinator.active_symbols_count(), 0);
        assert_eq!(coordinator.total_symbols_count(), 0);
    }

    #[test]
    fn test_coordinator_tick_update() {
        let config = CoordinatorConfig::default();
        let mut coordinator = SymbolCoordinator::new(config);

        coordinator.update_tick(100);
        assert_eq!(coordinator.current_tick(), 100);
    }

    #[test]
    fn test_ensure_active_placeholder() {
        let config = CoordinatorConfig::default();
        let coordinator = SymbolCoordinator::new(config);

        // Test the placeholder implementation
        let result = coordinator.ensure_active(1);
        assert!(result.is_ok());

        let ready_at_tick = result.unwrap();
        assert_eq!(ready_at_tick.next_tick, 0);
    }

    #[test]
    fn test_symbol_activation() {
        let config = CoordinatorConfig::default();
        let coordinator = SymbolCoordinator::new(config);

        // Test symbol activation
        let result = coordinator.ensure_active(1);
        assert!(result.is_ok());

        // Test that activating the same symbol again succeeds
        let result2 = coordinator.ensure_active(1);
        assert!(result2.is_ok());
    }

    #[test]
    fn test_thread_pool_creation() {
        let thread_pool = EngineThreadPool::new(4);

        assert_eq!(thread_pool.get_thread_load(0), Some(0));
        assert_eq!(thread_pool.get_thread_load(3), Some(0));
        assert_eq!(thread_pool.get_thread_load(4), None);
    }

    #[test]
    fn test_round_robin_policy() {
        let policy = RoundRobinPolicy::new(3);

        let thread1 = policy.assign_thread(1);
        let thread2 = policy.assign_thread(2);
        let thread3 = policy.assign_thread(3);
        let thread4 = policy.assign_thread(4);

        assert_eq!(thread1, 0);
        assert_eq!(thread2, 1);
        assert_eq!(thread3, 2);
        assert_eq!(thread4, 0); // Wraps around
    }

    #[test]
    fn test_hash_based_policy() {
        let policy = HashBasedPolicy::new(4);

        let thread1 = policy.assign_thread(1);
        let thread2 = policy.assign_thread(2);

        // Hash-based policy should be deterministic
        assert_eq!(policy.assign_thread(1), thread1);
        assert_eq!(policy.assign_thread(2), thread2);

        // Should be within bounds
        assert!(thread1 < 4);
        assert!(thread2 < 4);
    }

    #[test]
    fn test_queue_allocator() {
        let mut allocator = QueueAllocator::new(1024);

        let queue = allocator.create_queue();
        assert_eq!(queue.capacity(), 1024);

        allocator.preallocate_pool(5);
        let (pool_size, queue_depth) = allocator.pool_stats();
        assert_eq!(pool_size, 5);
        assert_eq!(queue_depth, 1024);
    }

    #[test]
    fn test_symbol_registry() {
        let mut registry = SymbolRegistry::new();

        // Test empty state
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        // Test symbol registration
        let queue = whistle::InboundQueue::new(1024);
        let result = registry.register_symbol(1, 0, queue);
        assert!(result.is_ok());

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_symbol_active(1));

        // Test symbol activation
        let result = registry.activate_symbol(1);
        assert!(result.is_ok());
        assert!(registry.is_symbol_active(1));

        // Test duplicate registration
        let queue2 = whistle::InboundQueue::new(1024);
        let result = registry.register_symbol(1, 0, queue2);
        assert!(result.is_err());
    }
}
