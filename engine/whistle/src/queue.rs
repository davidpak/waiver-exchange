// SPSC (Single Producer, Single Consumer) ring buffer for InboundMsg

use crate::{InboundMsg, RejectReason};
use std::cell::UnsafeCell;

/// SPSC ring buffer for inbound messages from OrderRouter
///
/// This implements a lock-free queue optimized for the order processing pipeline:
/// - Single producer (OrderRouter) enqueues messages
/// - Single consumer (Whistle) dequeues messages during tick processing
/// - Non-blocking backpressure: full queue causes immediate rejection
/// - Fixed capacity to prevent unbounded memory growth
/// - Atomic operations for thread safety
/// - Interior mutability for lock-free access
#[derive(Debug)]
pub struct InboundQueue {
    buffer: Box<[UnsafeCell<Option<InboundMsg>>]>,
    capacity: usize,
    mask: usize,
    head: std::sync::atomic::AtomicUsize,
    tail: std::sync::atomic::AtomicUsize,
}

// SAFETY: InboundQueue is safe to send and sync because:
// - All fields are Send + Sync
// - Atomic operations provide thread safety
// - UnsafeCell is used correctly for interior mutability
unsafe impl Send for InboundQueue {}
unsafe impl Sync for InboundQueue {}

impl InboundQueue {
    /// Create a new SPSC queue with the specified capacity
    ///
    /// Capacity must be a power of 2 for efficient modulo operations.
    /// This is enforced by rounding up to the next power of 2.
    pub fn new(capacity: usize) -> Self {
        let actual_capacity = capacity.next_power_of_two();
        let mut buffer = Vec::with_capacity(actual_capacity);
        for _ in 0..actual_capacity {
            buffer.push(UnsafeCell::new(None));
        }

        Self {
            buffer: buffer.into_boxed_slice(),
            capacity: actual_capacity,
            mask: actual_capacity - 1,
            head: std::sync::atomic::AtomicUsize::new(0),
            tail: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head.load(std::sync::atomic::Ordering::Acquire)
            == self.tail.load(std::sync::atomic::Ordering::Acquire)
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        let head = self.head.load(std::sync::atomic::Ordering::Acquire);
        let tail = self.tail.load(std::sync::atomic::Ordering::Acquire);

        (tail + 1) & self.mask == head
    }

    #[inline]
    pub fn len(&self) -> usize {
        let head = self.head.load(std::sync::atomic::Ordering::Acquire);
        let tail = self.tail.load(std::sync::atomic::Ordering::Acquire);

        if tail >= head { tail - head } else { self.capacity - (head - tail) }
    }

    /// Try to enqueue a message (producer operation)
    ///
    /// Returns:
    /// - `Ok(())` if the message was successfully enqueued
    /// - `Err(RejectReason::QueueBackpressure)` if the queue is full
    ///
    /// This operation is designed to be called by OrderRouter.
    #[inline]
    pub fn try_enqueue(&mut self, msg: InboundMsg) -> Result<(), RejectReason> {
        self.try_enqueue_lockfree(msg)
    }

    /// Lock-free enqueue operation (does not require mutable access)
    ///
    /// This is the same as try_enqueue but doesn't require &mut self,
    /// making it suitable for shared access in SPSC scenarios.
    #[inline]
    pub fn try_enqueue_lockfree(&self, msg: InboundMsg) -> Result<(), RejectReason> {
        let tail = self.tail.load(std::sync::atomic::Ordering::Acquire);
        let next_tail = (tail + 1) & self.mask;
        let head = self.head.load(std::sync::atomic::Ordering::Acquire);

        if next_tail == head {
            return Err(RejectReason::QueueBackpressure);
        }

        match self.tail.compare_exchange_weak(
            tail,
            next_tail,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        ) {
            Ok(_) => {
                unsafe {
                    let slot = self.buffer.get_unchecked(tail);
                    *slot.get() = Some(msg);
                }
                Ok(())
            }
            Err(_) => {
                // Another thread modified tail, retry or fail
                Err(RejectReason::QueueBackpressure)
            }
        }
    }

    /// Try to dequeue a message (consumer operation)
    ///
    /// Returns:
    /// - `Some(InboundMsg)` if a message was available
    /// - `None` if the queue is empty
    ///
    /// This operation is designed to be called by Whistle during tick processing.
    #[inline]
    pub fn try_dequeue(&mut self) -> Option<InboundMsg> {
        let head = self.head.load(std::sync::atomic::Ordering::Acquire);
        let tail = self.tail.load(std::sync::atomic::Ordering::Acquire);

        if head == tail {
            return None;
        }

        match self.head.compare_exchange_weak(
            head,
            (head + 1) & self.mask,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        ) {
            Ok(_) => unsafe {
                let slot = self.buffer.get_unchecked(head);
                (*slot.get()).take()
            },
            Err(_) => None,
        }
    }

    /// Lock-free dequeue operation (does not require mutable access)
    ///
    /// This is the same as try_dequeue but doesn't require &mut self,
    /// making it suitable for shared access in SPSC scenarios.
    #[inline]
    pub fn try_dequeue_lockfree(&self) -> Option<InboundMsg> {
        let head = self.head.load(std::sync::atomic::Ordering::Acquire);
        let tail = self.tail.load(std::sync::atomic::Ordering::Acquire);

        if head == tail {
            return None;
        }

        match self.head.compare_exchange_weak(
            head,
            (head + 1) & self.mask,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        ) {
            Ok(_) => unsafe {
                let slot = self.buffer.get_unchecked(head);
                (*slot.get()).take()
            },
            Err(_) => None,
        }
    }

    /// Drain up to `max_messages` from the queue
    ///
    /// This is the main consumer operation used during tick processing.
    /// Returns the number of messages actually dequeued.
    pub fn drain(&mut self, max_messages: usize) -> Vec<InboundMsg> {
        let mut messages = Vec::with_capacity(max_messages.min(self.len()));

        for _ in 0..max_messages {
            match self.try_dequeue() {
                Some(msg) => messages.push(msg),
                None => break, // Queue is empty
            }
        }

        messages
    }

    /// Clear all messages from the queue
    ///
    /// This is useful for testing and error recovery scenarios.
    pub fn clear(&mut self) {
        while self.try_dequeue().is_some() {
            // Drain all messages
        }
    }

    /// Lock-free drain operation (does not require mutable access)
    ///
    /// This is the same as drain but doesn't require &mut self,
    /// making it suitable for shared access in SPSC scenarios.
    pub fn drain_lockfree(&self, max_messages: usize) -> Vec<InboundMsg> {
        let mut messages = Vec::with_capacity(max_messages.min(self.len()));

        for _ in 0..max_messages {
            match self.try_dequeue_lockfree() {
                Some(msg) => messages.push(msg),
                None => break, // Queue is empty
            }
        }

        messages
    }

    /// Lock-free clear operation (does not require mutable access)
    ///
    /// This is the same as clear but doesn't require &mut self,
    /// making it suitable for shared access in SPSC scenarios.
    pub fn clear_lockfree(&self) {
        while self.try_dequeue_lockfree().is_some() {
            // Drain all messages
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OrderType, Side};

    #[test]
    fn test_queue_creation() {
        let queue = InboundQueue::new(16);
        assert_eq!(queue.capacity(), 16);
        assert!(queue.is_empty());
        assert!(!queue.is_full());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_queue_power_of_two_rounding() {
        let queue = InboundQueue::new(10);
        assert_eq!(queue.capacity(), 16);
    }

    #[test]
    fn test_enqueue_dequeue_single() {
        let mut queue = InboundQueue::new(4);

        let msg = InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(100), 10, 1000, 0, 1);

        assert!(queue.try_enqueue(msg).is_ok());
        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 1);

        let dequeued = queue.try_dequeue();
        assert!(dequeued.is_some());
        assert_eq!(dequeued.unwrap().order_id(), 1);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_full_backpressure() {
        let mut queue = InboundQueue::new(2);

        let msg1 = InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(100), 10, 1000, 0, 1);
        let msg2 = InboundMsg::submit(2, 1, Side::Buy, OrderType::Limit, Some(100), 10, 1000, 0, 2);
        let msg3 = InboundMsg::submit(3, 1, Side::Buy, OrderType::Limit, Some(100), 10, 1000, 0, 3);

        assert!(queue.try_enqueue(msg1).is_ok());
        assert!(queue.is_full());

        assert_eq!(queue.try_enqueue(msg2), Err(RejectReason::QueueBackpressure));

        assert_eq!(queue.try_enqueue(msg3), Err(RejectReason::QueueBackpressure));
    }

    #[test]
    fn test_lockfree_interface() {
        let queue = InboundQueue::new(4);
        let msg = InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(100), 10, 1000, 0, 1);

        // Test that we can enqueue using the lock-free interface
        assert!(queue.try_enqueue_lockfree(msg).is_ok());
        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_drain_messages() {
        let mut queue = InboundQueue::new(8);

        for i in 0..5 {
            let msg = InboundMsg::submit(
                i,
                1,
                Side::Buy,
                OrderType::Limit,
                Some(100),
                10,
                1000,
                0,
                i as u32,
            );
            assert!(queue.try_enqueue(msg).is_ok());
        }

        assert_eq!(queue.len(), 5);

        let drained = queue.drain(3);
        assert_eq!(drained.len(), 3);
        assert_eq!(queue.len(), 2);

        let remaining = queue.drain(10);
        assert_eq!(remaining.len(), 2);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_clear_queue() {
        let mut queue = InboundQueue::new(4);

        for i in 0..3 {
            let msg = InboundMsg::submit(
                i,
                1,
                Side::Buy,
                OrderType::Limit,
                Some(100),
                10,
                1000,
                0,
                i as u32,
            );
            assert!(queue.try_enqueue(msg).is_ok());
        }

        assert_eq!(queue.len(), 3);

        queue.clear();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }
}
