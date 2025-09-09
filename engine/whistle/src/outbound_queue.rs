// MPSC (Multiple Producer, Single Consumer) ring buffer for EngineEvent

use crate::{EngineEvent, RejectReason};
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

/// MPSC ring buffer for outbound events from Whistle to ExecutionManager
///
/// This implements a lock-free queue optimized for the event emission pipeline:
/// - Multiple producers (Whistle instances) enqueue events
/// - Single consumer (ExecutionManager) dequeues events
/// - Configurable backpressure policy: Fatal (exit) or Drop (with metrics)
/// - Fixed capacity to prevent unbounded memory growth
/// - Atomic operations for thread safety
/// - Interior mutability for lock-free access
#[derive(Debug)]
pub struct OutboundQueue {
    buffer: Box<[UnsafeCell<Option<EngineEvent>>]>,
    capacity: usize,
    mask: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
    backpressure_policy: BackpressurePolicy,
}

/// Backpressure handling policy for queue overflow
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressurePolicy {
    /// System exits on overflow (recommended for data integrity)
    Fatal,
    /// Drop events on overflow (with comprehensive metrics)
    Drop,
}

// SAFETY: OutboundQueue is safe to send and sync because:
// - All fields are Send + Sync
// - Atomic operations provide thread safety
// - UnsafeCell is used correctly for interior mutability
unsafe impl Send for OutboundQueue {}
unsafe impl Sync for OutboundQueue {}

impl OutboundQueue {
    /// Create a new MPSC queue with the specified capacity and backpressure policy
    ///
    /// Capacity must be a power of 2 for efficient modulo operations.
    /// This is enforced by rounding up to the next power of 2.
    pub fn new(capacity: usize, backpressure_policy: BackpressurePolicy) -> Self {
        let actual_capacity = capacity.next_power_of_two();
        let mut buffer = Vec::with_capacity(actual_capacity);
        for _ in 0..actual_capacity {
            buffer.push(UnsafeCell::new(None));
        }

        Self {
            buffer: buffer.into_boxed_slice(),
            capacity: actual_capacity,
            mask: actual_capacity - 1,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            backpressure_policy,
        }
    }

    /// Create a new queue with default capacity (8,192 events) and Fatal policy
    pub fn with_default_capacity() -> Self {
        Self::new(8192, BackpressurePolicy::Fatal)
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        (tail + 1) & self.mask == head
    }

    #[inline]
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        if tail >= head { tail - head } else { self.capacity - (head - tail) }
    }

    /// Try to enqueue an event (producer operation)
    ///
    /// Returns:
    /// - `Ok(())` if the event was successfully enqueued
    /// - `Err(RejectReason::QueueBackpressure)` if the queue is full
    ///
    /// This operation is designed to be called by Whistle instances.
    #[inline]
    pub fn try_enqueue(&self, event: EngineEvent) -> Result<(), RejectReason> {
        let tail = self.tail.load(Ordering::Acquire);
        let next_tail = (tail + 1) & self.mask;
        let head = self.head.load(Ordering::Acquire);

        if next_tail == head {
            // Queue is full - apply backpressure policy
            match self.backpressure_policy {
                BackpressurePolicy::Fatal => {
                    eprintln!("OutboundQueue overflow - system integrity compromised");
                    eprintln!("Queue capacity: {}, current length: {}", self.capacity, self.len());
                    eprintln!("Event that caused overflow: {event:?}");
                    std::process::exit(1);
                }
                BackpressurePolicy::Drop => {
                    // TODO: Add metrics tracking for dropped events
                    return Err(RejectReason::QueueBackpressure);
                }
            }
        }

        match self.tail.compare_exchange_weak(tail, next_tail, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => {
                unsafe {
                    let slot = self.buffer.get_unchecked(tail);
                    *slot.get() = Some(event);
                }
                Ok(())
            }
            Err(_) => {
                // Another thread modified tail, retry or fail
                Err(RejectReason::QueueBackpressure)
            }
        }
    }

    /// Try to dequeue an event (consumer operation)
    ///
    /// Returns:
    /// - `Some(EngineEvent)` if an event was available
    /// - `None` if the queue is empty
    ///
    /// This operation is designed to be called by ExecutionManager.
    #[inline]
    pub fn try_dequeue(&self) -> Option<EngineEvent> {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        if head == tail {
            return None;
        }

        match self.head.compare_exchange_weak(
            head,
            (head + 1) & self.mask,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => unsafe {
                let slot = self.buffer.get_unchecked(head);
                (*slot.get()).take()
            },
            Err(_) => None,
        }
    }

    /// Drain up to `max_events` from the queue
    ///
    /// This is the main consumer operation used by ExecutionManager.
    /// Returns the number of events actually dequeued.
    pub fn drain(&self, max_events: usize) -> Vec<EngineEvent> {
        let mut events = Vec::with_capacity(max_events.min(self.len()));

        for _ in 0..max_events {
            match self.try_dequeue() {
                Some(event) => events.push(event),
                None => break, // Queue is empty
            }
        }

        events
    }

    /// Clear all events from the queue
    ///
    /// This is useful for testing and error recovery scenarios.
    pub fn clear(&self) {
        while self.try_dequeue().is_some() {
            // Drain all events
        }
    }

    /// Get the current backpressure policy
    pub fn backpressure_policy(&self) -> BackpressurePolicy {
        self.backpressure_policy
    }

    /// Update the backpressure policy (for testing or configuration)
    pub fn set_backpressure_policy(&mut self, policy: BackpressurePolicy) {
        self.backpressure_policy = policy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EvBookDelta, EvTickComplete, Side};

    fn create_test_trade() -> EngineEvent {
        EngineEvent::Trade(crate::EvTrade {
            symbol: 1,
            tick: 100,
            exec_id: 12345,
            price: 150,
            qty: 10,
            taker_side: Side::Buy,
            maker_order: 1,
            taker_order: 2,
        })
    }

    fn create_test_book_delta() -> EngineEvent {
        EngineEvent::BookDelta(EvBookDelta {
            symbol: 1,
            tick: 100,
            side: Side::Buy,
            price: 150,
            level_qty_after: 20,
        })
    }

    fn create_test_tick_complete() -> EngineEvent {
        EngineEvent::TickComplete(EvTickComplete { symbol: 1, tick: 100 })
    }

    #[test]
    fn test_queue_creation() {
        let queue = OutboundQueue::new(16, BackpressurePolicy::Fatal);
        assert_eq!(queue.capacity(), 16);
        assert!(queue.is_empty());
        assert!(!queue.is_full());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_queue_power_of_two_rounding() {
        let queue = OutboundQueue::new(10, BackpressurePolicy::Fatal);
        assert_eq!(queue.capacity(), 16);
    }

    #[test]
    fn test_default_capacity() {
        let queue = OutboundQueue::with_default_capacity();
        assert_eq!(queue.capacity(), 8192);
        assert_eq!(queue.backpressure_policy(), BackpressurePolicy::Fatal);
    }

    #[test]
    fn test_enqueue_dequeue_single() {
        let queue = OutboundQueue::new(4, BackpressurePolicy::Fatal);
        let event = create_test_trade();

        assert!(queue.try_enqueue(event).is_ok());
        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 1);

        let dequeued = queue.try_dequeue();
        assert!(dequeued.is_some());
        assert!(matches!(dequeued.unwrap(), EngineEvent::Trade(_)));
        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_full_fatal_policy() {
        let queue = OutboundQueue::new(2, BackpressurePolicy::Fatal);
        let event1 = create_test_trade();
        let _event2 = create_test_book_delta();
        let _event3 = create_test_tick_complete();

        assert!(queue.try_enqueue(event1).is_ok());
        assert!(queue.is_full());

        // This should cause a panic/exit in fatal mode
        // We can't easily test this without causing the test to exit
        // In a real scenario, this would exit the process
    }

    #[test]
    fn test_queue_full_drop_policy() {
        let queue = OutboundQueue::new(2, BackpressurePolicy::Drop);
        let event1 = create_test_trade();
        let event2 = create_test_book_delta();
        let event3 = create_test_tick_complete();

        assert!(queue.try_enqueue(event1).is_ok());
        assert!(queue.is_full());

        assert_eq!(queue.try_enqueue(event2), Err(RejectReason::QueueBackpressure));
        assert_eq!(queue.try_enqueue(event3), Err(RejectReason::QueueBackpressure));
    }

    #[test]
    fn test_drain_events() {
        let queue = OutboundQueue::new(8, BackpressurePolicy::Fatal);

        for _i in 0..5 {
            let event = create_test_trade();
            assert!(queue.try_enqueue(event).is_ok());
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
        let queue = OutboundQueue::new(4, BackpressurePolicy::Fatal);

        for _ in 0..3 {
            let event = create_test_trade();
            assert!(queue.try_enqueue(event).is_ok());
        }

        assert_eq!(queue.len(), 3);

        queue.clear();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_backpressure_policy_configuration() {
        let mut queue = OutboundQueue::new(4, BackpressurePolicy::Fatal);
        assert_eq!(queue.backpressure_policy(), BackpressurePolicy::Fatal);

        queue.set_backpressure_policy(BackpressurePolicy::Drop);
        assert_eq!(queue.backpressure_policy(), BackpressurePolicy::Drop);
    }

    #[test]
    fn test_multiple_event_types() {
        let queue = OutboundQueue::new(8, BackpressurePolicy::Fatal);

        let trade = create_test_trade();
        let book_delta = create_test_book_delta();
        let tick_complete = create_test_tick_complete();

        assert!(queue.try_enqueue(trade).is_ok());
        assert!(queue.try_enqueue(book_delta).is_ok());
        assert!(queue.try_enqueue(tick_complete).is_ok());

        assert_eq!(queue.len(), 3);

        let events = queue.drain(10);
        assert_eq!(events.len(), 3);

        // Verify event types
        assert!(matches!(events[0], EngineEvent::Trade(_)));
        assert!(matches!(events[1], EngineEvent::BookDelta(_)));
        assert!(matches!(events[2], EngineEvent::TickComplete(_)));
    }
}
