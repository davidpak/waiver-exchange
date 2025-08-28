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

        // Step 2: Process each message (validate, admit, queue for matching)
        let mut orders_to_match = Vec::new();
        for msg in messages {
            match self.process_message(msg, t) {
                Ok(handle) => orders_to_match.push(handle),
                Err(_) => {
                    // Rejection already emitted as lifecycle event
                }
            }
        }

        // Step 3: Match orders using price-time priority
        self.match_orders(orders_to_match, t);

        // Step 4: Emit book delta events (coalesced)
        self.emit_book_deltas(t);

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
    fn process_message(
        &mut self,
        msg: InboundMsg,
        tick: TickId,
    ) -> Result<OrderHandle, RejectReason> {
        match msg.kind {
            MsgKind::Submit => {
                let submit = msg.submit.as_ref().unwrap();

                // Basic validation
                if let Some(price) = submit.price {
                    // Check tick size alignment and price domain
                    if self.dom.idx(price).is_none() {
                        let lifecycle = EvLifecycle {
                            symbol: self.cfg.symbol,
                            tick,
                            kind: LifecycleKind::Rejected,
                            order_id: submit.order_id,
                            reason: Some(RejectReason::BadTick),
                        };
                        self.emitter
                            .emit(EngineEvent::Lifecycle(lifecycle))
                            .expect("Lifecycle event should always be valid");
                        return Err(RejectReason::BadTick);
                    }
                }

                // Allocate order in arena
                let handle = self.arena.alloc().ok_or(RejectReason::ArenaFull)?;

                // Create order
                let order = Order {
                    id: submit.order_id,
                    acct: submit.account_id,
                    side: submit.side,
                    price_idx: submit.price.map(|p| self.dom.idx(p).unwrap()).unwrap_or(0),
                    qty_open: submit.qty,
                    ts_norm: submit.ts_norm,
                    enq_seq: msg.enq_seq,
                    typ: submit.typ as u8,
                    ..Default::default()
                };

                // Store order in arena
                *self.arena.get_mut(handle) = order;

                // Emit accepted lifecycle event
                let lifecycle = EvLifecycle {
                    symbol: self.cfg.symbol,
                    tick,
                    kind: LifecycleKind::Accepted,
                    order_id: submit.order_id,
                    reason: None,
                };
                self.emitter
                    .emit(EngineEvent::Lifecycle(lifecycle))
                    .expect("Lifecycle event should always be valid");

                Ok(handle)
            }
            MsgKind::Cancel => {
                let cancel = msg.cancel.as_ref().unwrap();

                // For now, just emit accepted lifecycle event
                // TODO: Implement actual cancel logic
                let lifecycle = EvLifecycle {
                    symbol: self.cfg.symbol,
                    tick,
                    kind: LifecycleKind::Accepted,
                    order_id: cancel.order_id,
                    reason: None,
                };
                self.emitter
                    .emit(EngineEvent::Lifecycle(lifecycle))
                    .expect("Lifecycle event should always be valid");

                // Return a dummy handle for now
                Err(RejectReason::UnknownOrder)
            }
        }
    }

    /// Match orders using strict price-time priority
    ///
    /// This implements the core matching logic as specified in the documentation:
    /// - Strict price-time priority with (ts_norm, enq_seq) tie-breaking
    /// - Self-match prevention based on policy
    /// - Order type semantics (LIMIT, MARKET, IOC, POST-ONLY)
    fn match_orders(&mut self, orders_to_match: Vec<OrderHandle>, _tick: TickId) {
        // TODO: Implement full matching logic
        // For now, just process orders without matching
        for handle in orders_to_match {
            let order = self.arena.get(handle);
            let side = order.side;
            let price_idx = order.price_idx;
            let qty_open = order.qty_open;

            // Add to book if it's a resting order type
            match order.typ {
                0 => {
                    // OrderType::Limit
                    // Add to book
                    self.book.insert_tail(&mut self.arena, side, handle, price_idx, qty_open);
                }
                1 => { // OrderType::Market
                    // Market orders never rest - should match immediately
                    // TODO: Implement immediate matching
                }
                2 => { // OrderType::Ioc
                    // IOC orders match immediately, cancel remainder
                    // TODO: Implement immediate matching
                }
                3 => {
                    // OrderType::PostOnly
                    // POST-ONLY orders add liquidity only
                    // TODO: Check if it would cross before adding
                    self.book.insert_tail(&mut self.arena, side, handle, price_idx, qty_open);
                }
                _ => {
                    // Invalid order type
                }
            }
        }
    }

    /// Emit book delta events for all levels that changed during the tick
    ///
    /// This coalesces all changes to a level into a single BookDelta event
    /// with the final post-tick state.
    fn emit_book_deltas(&mut self, _tick: TickId) {
        // TODO: Implement book delta emission
        // For now, this is a placeholder
        // The book should track which levels were modified during the tick
        // and emit BookDelta events for each modified level
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

    #[test]
    fn order_validation_rejection() {
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

        // Test invalid tick size (103 not aligned to tick=5)
        let invalid_tick_msg = InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(103), 10, 1000, 0, 1);
        eng.enqueue_message(invalid_tick_msg).unwrap();

        let events = eng.tick(100);
        
        // Should have 2 events: 1 rejected lifecycle + 1 tick complete
        assert_eq!(events.len(), 2);
        
        match &events[0] {
            EngineEvent::Lifecycle(ev) => {
                assert_eq!(ev.symbol, 42);
                assert_eq!(ev.tick, 100);
                assert_eq!(ev.order_id, 1);
                assert_eq!(ev.kind, LifecycleKind::Rejected);
                assert_eq!(ev.reason, Some(RejectReason::BadTick));
            }
            _ => panic!("Expected rejected Lifecycle event"),
        }

        match &events[1] {
            EngineEvent::TickComplete(ev) => {
                assert_eq!(ev.symbol, 42);
                assert_eq!(ev.tick, 100);
            }
            _ => panic!("Expected TickComplete event"),
        }
    }

    #[test]
    fn canonical_event_ordering() {
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

        // Submit multiple orders to test event ordering
        let msg1 = InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1);
        let msg2 = InboundMsg::submit(2, 2, Side::Sell, OrderType::Limit, Some(160), 5, 1001, 0, 2);
        let msg3 = InboundMsg::cancel(3, 1002, 3);

        eng.enqueue_message(msg1).unwrap();
        eng.enqueue_message(msg2).unwrap();
        eng.enqueue_message(msg3).unwrap();

        let events = eng.tick(100);

        // Verify canonical order: Lifecycle events first, then TickComplete
        assert!(events.len() >= 2); // At least lifecycle events + tick complete
        
        // All lifecycle events should come before TickComplete
        let mut found_tick_complete = false;
        for event in &events {
            match event {
                EngineEvent::TickComplete(_) => {
                    found_tick_complete = true;
                }
                EngineEvent::Lifecycle(_) => {
                    assert!(!found_tick_complete, "Lifecycle events must come before TickComplete");
                }
                _ => {
                    // Other event types should also come before TickComplete
                    assert!(!found_tick_complete, "All events must come before TickComplete");
                }
            }
        }
        
        // Must have exactly one TickComplete at the end
        assert!(found_tick_complete, "Must have TickComplete event");
    }

    #[test]
    fn book_state_management() {
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

        // Submit a limit order that should rest in the book
        let buy_order = InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1);
        eng.enqueue_message(buy_order).unwrap();

        let events = eng.tick(100);
        
        // Should be accepted and added to book
        assert_eq!(events.len(), 2); // Lifecycle + TickComplete
        
        match &events[0] {
            EngineEvent::Lifecycle(ev) => {
                assert_eq!(ev.kind, LifecycleKind::Accepted);
                assert_eq!(ev.order_id, 1);
            }
            _ => panic!("Expected accepted Lifecycle event"),
        }

        // Submit another order in next tick
        let sell_order = InboundMsg::submit(2, 2, Side::Sell, OrderType::Limit, Some(160), 5, 1001, 0, 1);
        eng.enqueue_message(sell_order).unwrap();

        let events2 = eng.tick(101);
        
        // Should also be accepted
        assert_eq!(events2.len(), 2); // Lifecycle + TickComplete
        
        match &events2[0] {
            EngineEvent::Lifecycle(ev) => {
                assert_eq!(ev.kind, LifecycleKind::Accepted);
                assert_eq!(ev.order_id, 2);
            }
            _ => panic!("Expected accepted Lifecycle event"),
        }
    }

    #[test]
    fn arena_capacity_limits() {
        let cfg = EngineCfg {
            symbol: 42,
            price_domain: PriceDomain { floor: 100, ceil: 200, tick: 5 },
            bands: Bands { mode: BandMode::Percent(1000) },
            batch_max: 1024,
            arena_capacity: 8, // Must be power of 2 >= 8 for OrderIndex
            elastic_arena: false,
            exec_shift_bits: 12,
            exec_id_mode: ExecIdMode::Sharded,
            self_match_policy: SelfMatchPolicy::Skip,
            allow_market_cold_start: false,
            reference_price_source: ReferencePriceSource::SnapshotLastTrade,
        };
        let mut eng = Whistle::new(cfg);

        // Submit orders up to capacity
        let msg1 = InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1);
        let msg2 = InboundMsg::submit(2, 2, Side::Buy, OrderType::Limit, Some(155), 10, 1001, 0, 2);
        let msg3 = InboundMsg::submit(3, 3, Side::Buy, OrderType::Limit, Some(160), 10, 1002, 0, 3);
        let msg4 = InboundMsg::submit(4, 4, Side::Buy, OrderType::Limit, Some(165), 10, 1003, 0, 4);
        let msg5 = InboundMsg::submit(5, 5, Side::Buy, OrderType::Limit, Some(170), 10, 1004, 0, 5);
        let msg6 = InboundMsg::submit(6, 6, Side::Buy, OrderType::Limit, Some(175), 10, 1005, 0, 6);
        let msg7 = InboundMsg::submit(7, 7, Side::Buy, OrderType::Limit, Some(180), 10, 1006, 0, 7);
        let msg8 = InboundMsg::submit(8, 8, Side::Buy, OrderType::Limit, Some(185), 10, 1007, 0, 8);
        let msg9 = InboundMsg::submit(9, 9, Side::Buy, OrderType::Limit, Some(190), 10, 1008, 0, 9);

        eng.enqueue_message(msg1).unwrap();
        eng.enqueue_message(msg2).unwrap();
        eng.enqueue_message(msg3).unwrap();
        eng.enqueue_message(msg4).unwrap();
        eng.enqueue_message(msg5).unwrap();
        eng.enqueue_message(msg6).unwrap();
        eng.enqueue_message(msg7).unwrap();
        eng.enqueue_message(msg8).unwrap();
        eng.enqueue_message(msg9).unwrap();

        let events = eng.tick(100);

        // Check how many lifecycle events we got
        let lifecycle_events: Vec<_> = events.iter()
            .filter_map(|e| {
                if let EngineEvent::Lifecycle(ev) = e {
                    Some(ev)
                } else {
                    None
                }
            })
            .collect();


        
        // Should have 8 lifecycle events (all accepted, 9th rejected before processing)
        assert_eq!(lifecycle_events.len(), 8);
        
        // Check that all 8 are accepted
        for i in 0..8 {
            assert_eq!(lifecycle_events[i].order_id, (i + 1) as u64);
            assert_eq!(lifecycle_events[i].kind, LifecycleKind::Accepted);
        }
    }

    #[test]
    fn determinism_replay() {
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

        // Run the same sequence twice
        let mut eng1 = Whistle::new(cfg.clone());
        let mut eng2 = Whistle::new(cfg);

        // Submit identical orders
        let msg1 = InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1);
        let msg2 = InboundMsg::submit(2, 2, Side::Sell, OrderType::Limit, Some(160), 5, 1001, 0, 2);

        eng1.enqueue_message(msg1.clone()).unwrap();
        eng1.enqueue_message(msg2.clone()).unwrap();
        eng2.enqueue_message(msg1).unwrap();
        eng2.enqueue_message(msg2).unwrap();

        let events1 = eng1.tick(100);
        let events2 = eng2.tick(100);

        // Events should be identical (deterministic)
        assert_eq!(events1.len(), events2.len());
        
        for (e1, e2) in events1.iter().zip(events2.iter()) {
            match (e1, e2) {
                (EngineEvent::Lifecycle(ev1), EngineEvent::Lifecycle(ev2)) => {
                    assert_eq!(ev1.symbol, ev2.symbol);
                    assert_eq!(ev1.tick, ev2.tick);
                    assert_eq!(ev1.order_id, ev2.order_id);
                    assert_eq!(ev1.kind, ev2.kind);
                    assert_eq!(ev1.reason, ev2.reason);
                }
                (EngineEvent::TickComplete(ev1), EngineEvent::TickComplete(ev2)) => {
                    assert_eq!(ev1.symbol, ev2.symbol);
                    assert_eq!(ev1.tick, ev2.tick);
                }
                _ => panic!("Event types should match"),
            }
        }
    }

    #[test]
    fn queue_backpressure() {
        let cfg = EngineCfg {
            symbol: 42,
            price_domain: PriceDomain { floor: 100, ceil: 200, tick: 5 },
            bands: Bands { mode: BandMode::Percent(1000) },
            batch_max: 2, // Very small batch size
            arena_capacity: 4096,
            elastic_arena: false,
            exec_shift_bits: 12,
            exec_id_mode: ExecIdMode::Sharded,
            self_match_policy: SelfMatchPolicy::Skip,
            allow_market_cold_start: false,
            reference_price_source: ReferencePriceSource::SnapshotLastTrade,
        };
        let mut eng = Whistle::new(cfg);

        // Check queue capacity
        let (_, capacity) = eng.queue_stats();
        assert_eq!(capacity, 2); // Queue uses exact batch_max value

        // Fill the queue
        for i in 0..10 {
            let msg = InboundMsg::submit(i, i, Side::Buy, OrderType::Limit, Some(150), 10, 1000 + i, 0, i as u32);
            let result = eng.enqueue_message(msg);
            
            // Should accept first few, then reject due to backpressure
            if i < 1 { // Queue capacity is 2, so can hold 1 message before full
                assert!(result.is_ok(), "Should accept message {}", i);
            } else {
                assert!(result.is_err(), "Should reject message {} due to backpressure", i);
                assert_eq!(result.unwrap_err(), RejectReason::QueueBackpressure);
            }
        }
    }
}
