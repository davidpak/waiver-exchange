use whistle::InboundQueue;

/// Allocator for SPSC queues
#[derive(Debug)]
pub struct QueueAllocator {
    spsc_depth: usize,
    queue_pool: Vec<InboundQueue>,
}

impl QueueAllocator {
    pub fn new(spsc_depth: usize) -> Self {
        Self { spsc_depth, queue_pool: Vec::new() }
    }

    /// Create a new SPSC queue for a symbol
    pub fn create_queue(&mut self) -> InboundQueue {
        InboundQueue::new(self.spsc_depth)
    }

    /// Get the configured queue depth
    pub fn queue_depth(&self) -> usize {
        self.spsc_depth
    }

    /// Pre-allocate a pool of queues (for performance optimization)
    pub fn preallocate_pool(&mut self, count: usize) {
        for _ in 0..count {
            let queue = InboundQueue::new(self.spsc_depth);
            self.queue_pool.push(queue);
        }
    }

    /// Get a queue from the pool if available
    pub fn get_from_pool(&mut self) -> Option<InboundQueue> {
        self.queue_pool.pop()
    }

    /// Return a queue to the pool (for reuse)
    pub fn return_to_pool(&mut self, queue: InboundQueue) {
        // Note: In a real implementation, we'd want to ensure the queue is empty
        // and reset its state before returning to pool
        self.queue_pool.push(queue);
    }

    /// Get pool statistics
    pub fn pool_stats(&self) -> (usize, usize) {
        (self.queue_pool.len(), self.spsc_depth)
    }
}
