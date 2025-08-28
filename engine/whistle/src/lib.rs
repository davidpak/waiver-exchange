// Whistle - per-symbol deterministic matching engine
#![allow(dead_code)]

mod arena;
mod bitset;
mod book;
mod config;
mod emitter;
mod events;
mod messages;
mod order_index;
mod price_domain;
mod queue;
mod types;

pub use arena::{Arena, Order};
pub use bitset::Bitset;
pub use book::{Book, Level};
pub use config::{BandMode, Bands, EngineCfg, ExecIdMode, ReferencePriceSource, SelfMatchPolicy};
pub use emitter::{EmitError, EventEmitter};
pub use events::{
    EngineEvent, EvBookDelta, EvLifecycle, EvTickComplete, EvTrade, EventKind, LifecycleKind,
    RejectReason,
};
pub use messages::{Cancel, InboundMsg, MsgKind, Submit};
pub use order_index::OrderIndex;
pub use price_domain::{Price, PriceDomain, PriceIdx};
pub use queue::InboundQueue;
pub use types::{AccountId, EnqSeq, H_NONE, OrderHandle, OrderId, OrderType, Qty, Side, TsNorm};

pub type TickId = u64;

pub struct Whistle {
    cfg: EngineCfg,
    dom: PriceDomain,
    emitter: EventEmitter,
    seq_in_tick: u32, // Sequence number for events within the current tick

    // Order processing components
    inbound_queue: InboundQueue, // SPSC queue for messages from OrderRouter
    arena: Arena,                // Preallocated order storage
    book: Book,                  // Order book with price-time priority
    order_index: OrderIndex,     // O(1) order lookup by order_id

    // State tracking
    reference_price: Option<Price>, // Current reference price for bands
}

impl Whistle {
    pub fn new(cfg: EngineCfg) -> Self {
        cfg.validate().expect("invalid EngineCfg");
        let dom = cfg.price_domain;
        let emitter = EventEmitter::new(cfg.symbol);

        // Initialize order processing components
        let inbound_queue = InboundQueue::new(cfg.batch_max as usize);
        let arena = Arena::with_capacity(cfg.arena_capacity);
        let book = Book::new(dom);
        let order_index = OrderIndex::with_capacity_pow2(cfg.arena_capacity as usize * 2); // 2x capacity for hash table

        Self {
            cfg,
            dom,
            emitter,
            seq_in_tick: 0,
            inbound_queue,
            arena,
            book,
            order_index,
            reference_price: None,
        }
    }

    /// Process a tick - the core matching engine entry point
    /// This is where all order processing and matching occurs
    pub fn tick(&mut self, t: TickId) -> Vec<EngineEvent> {
        // Start new tick - reset sequence counter and emitter
        self.seq_in_tick = 0;
        self.emitter.start_tick(t);

        // Step 1: Drain inbound queue (up to batch_max)
        let messages = self.inbound_queue.drain(self.cfg.batch_max as usize);

        // Step 2: Process each message
        for msg in messages {
            self.process_message(msg, t);
        }

        // TODO: Step 3: Match orders using price-time priority
        // TODO: Step 4: Emit trade events
        // TODO: Step 5: Emit book delta events
        // TODO: Step 6: Emit lifecycle events

        // Emit tick complete event
        let tick_complete = EvTickComplete { symbol: self.cfg.symbol, tick: t };
        self.emitter
            .emit(EngineEvent::TickComplete(tick_complete))
            .expect("TickComplete should always be valid");

        // Return all events for this tick
        self.emitter.take_events()
    }

    #[inline]
    pub fn price_domain(&self) -> &PriceDomain {
        &self.dom
    }

    /// Get the next execution ID for this tick
    #[inline]
    pub fn next_exec_id(&mut self, tick: TickId) -> u64 {
        let exec_id = (tick << self.cfg.exec_shift_bits) | (self.seq_in_tick as u64);
        self.seq_in_tick += 1;
        exec_id
    }

    /// Process a single inbound message
    ///
    /// This handles the initial message processing pipeline:
    /// 1. Basic validation (message structure, price domain)
    /// 2. Emit lifecycle events for all messages (accepted/rejected)
    /// 3. Queue valid orders for matching (future implementation)
    fn process_message(&mut self, msg: InboundMsg, tick: TickId) {
        // For now, emit lifecycle events for all messages
        // TODO: Add proper validation and order admission logic

        let lifecycle = EvLifecycle {
            symbol: self.cfg.symbol,
            tick,
            kind: LifecycleKind::Accepted, // TODO: Determine based on validation
            order_id: msg.order_id(),
            reason: None, // TODO: Set rejection reason if validation fails
        };

        self.emitter
            .emit(EngineEvent::Lifecycle(lifecycle))
            .expect("Lifecycle event should always be valid");
    }

    /// Enqueue a message from OrderRouter
    ///
    /// This is the public interface for OrderRouter to send messages to Whistle.
    /// Returns:
    /// - `Ok(())` if the message was successfully enqueued
    /// - `Err(RejectReason::QueueBackpressure)` if the queue is full
    pub fn enqueue_message(&mut self, msg: InboundMsg) -> Result<(), RejectReason> {
        self.inbound_queue.try_enqueue(msg)
    }

    /// Get queue statistics for monitoring
    pub fn queue_stats(&self) -> (usize, usize) {
        (self.inbound_queue.len(), self.inbound_queue.capacity())
    }

    /// Clear the inbound queue
    pub fn clear_queue(&mut self) {
        self.inbound_queue.clear();
    }

    /// Get the symbol ID
    #[inline]
    pub fn symbol(&self) -> u32 {
        self.cfg.symbol
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn price_domain_roundtrip() {
        let cfg = EngineCfg {
            symbol: 1,
            price_domain: PriceDomain { floor: 100, ceil: 200, tick: 5 },
            bands: Bands { mode: BandMode::Percent(1000) }, // +/-10.00%
            batch_max: 1024,
            arena_capacity: 4096,
            elastic_arena: false,
            exec_shift_bits: 12,
            exec_id_mode: ExecIdMode::Sharded,
            self_match_policy: SelfMatchPolicy::Skip,
            allow_market_cold_start: false,
            reference_price_source: ReferencePriceSource::SnapshotLastTrade,
        };
        let eng = Whistle::new(cfg);
        let dom = eng.price_domain();
        for p in [100, 105, 150, 200] {
            let i = dom.idx(p).expect("aligned");
            assert_eq!(dom.price(i), p);
        }
        assert!(dom.idx(103).is_none());
        assert_eq!(dom.ladder_len(), ((200 - 100) / 5) + 1);
    }

    #[test]
    fn basic_tick_functionality() {
        let cfg = EngineCfg {
            symbol: 42,
            price_domain: PriceDomain { floor: 100, ceil: 200, tick: 5 },
            bands: Bands { mode: BandMode::Percent(1000) },
            batch_max: 1024,
            arena_capacity: 4096,
            elastic_arena: false,
            exec_shift_bits: 12,
            exec_id_mode: ExecIdMode::Sharded,
            self_match_policy: SelfMatchPolicy::Skip,
            allow_market_cold_start: false,
            reference_price_source: ReferencePriceSource::SnapshotLastTrade,
        };
        let mut eng = Whistle::new(cfg);

        let events = eng.tick(100);

        assert_eq!(events.len(), 1);
        match &events[0] {
            EngineEvent::TickComplete(ev) => {
                assert_eq!(ev.symbol, 42);
                assert_eq!(ev.tick, 100);
            }
            _ => panic!("Expected TickComplete event"),
        }
    }

    #[test]
    fn message_processing_flow() {
        let cfg = EngineCfg {
            symbol: 42,
            price_domain: PriceDomain { floor: 100, ceil: 200, tick: 5 },
            bands: Bands { mode: BandMode::Percent(1000) },
            batch_max: 1024,
            arena_capacity: 4096,
            elastic_arena: false,
            exec_shift_bits: 12,
            exec_id_mode: ExecIdMode::Sharded,
            self_match_policy: SelfMatchPolicy::Skip,
            allow_market_cold_start: false,
            reference_price_source: ReferencePriceSource::SnapshotLastTrade,
        };
        let mut eng = Whistle::new(cfg);

        // Simulate OrderRouter enqueueing messages
        let msg1 = InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1);
        let msg2 = InboundMsg::cancel(2, 1001, 2);

        assert!(eng.enqueue_message(msg1).is_ok());
        assert!(eng.enqueue_message(msg2).is_ok());

        // Verify queue stats
        let (len, capacity) = eng.queue_stats();
        assert_eq!(len, 2);
        assert_eq!(capacity, 1024); // batch_max rounded to power of 2

        // Process tick - should drain messages and emit lifecycle events
        let events = eng.tick(100);

        // Should have 3 events: 2 lifecycle + 1 tick complete
        assert_eq!(events.len(), 3);

        // Check lifecycle events
        match &events[0] {
            EngineEvent::Lifecycle(ev) => {
                assert_eq!(ev.symbol, 42);
                assert_eq!(ev.tick, 100);
                assert_eq!(ev.order_id, 1);
                assert_eq!(ev.kind, LifecycleKind::Accepted);
            }
            _ => panic!("Expected Lifecycle event"),
        }

        match &events[1] {
            EngineEvent::Lifecycle(ev) => {
                assert_eq!(ev.symbol, 42);
                assert_eq!(ev.tick, 100);
                assert_eq!(ev.order_id, 2);
                assert_eq!(ev.kind, LifecycleKind::Accepted);
            }
            _ => panic!("Expected Lifecycle event"),
        }

        // Check tick complete event
        match &events[2] {
            EngineEvent::TickComplete(ev) => {
                assert_eq!(ev.symbol, 42);
                assert_eq!(ev.tick, 100);
            }
            _ => panic!("Expected TickComplete event"),
        }

        // Queue should be empty after processing
        let (len, _) = eng.queue_stats();
        assert_eq!(len, 0);
    }
}
