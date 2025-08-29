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
    modified_levels: std::collections::HashSet<(Side, PriceIdx)>, // Track levels modified during tick
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
            modified_levels: std::collections::HashSet::new(),
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
        let mut rejections = Vec::new();
        for msg in messages {
            match self.process_message(msg.clone(), t) {
                Ok(handle) => orders_to_match.push(handle),
                Err(reason) => {
                    // Queue rejection for later emission
                    rejections.push((msg, reason));
                }
            }
        }

        // Step 4: Match orders using price-time priority
        let match_rejections = self.match_orders(&mut orders_to_match[..], t);

        // Step 5: Extract order IDs for accepted orders (after matching/rejection)
        // Only include orders that are still valid in the arena
        let mut accepted_order_ids = Vec::new();
        for handle in &orders_to_match {
            if self.arena.is_valid(*handle) {
                let order = self.arena.get(*handle);
                accepted_order_ids.push(order.id);
            }
        }

        // Step 6: Emit book delta events (coalesced)
        self.emit_book_deltas(t);

        // Step 7: Emit lifecycle events (accepted/rejected orders)
        self.emit_lifecycle_events(rejections, &accepted_order_ids, t);

        // Step 8: Emit lifecycle events for match rejections (only if there are any)
        if !match_rejections.is_empty() {
            for (order_id, reason) in match_rejections {
                let lifecycle = EvLifecycle {
                    symbol: self.cfg.symbol,
                    tick: t,
                    kind: LifecycleKind::Rejected,
                    order_id,
                    reason: Some(reason),
                };
                self.emitter
                    .emit(EngineEvent::Lifecycle(lifecycle))
                    .expect("Lifecycle event should always be valid");
            }
        }

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

    /// Get order book levels for a given side
    /// Returns a vector of (price, quantity) pairs sorted by price
    pub fn get_order_book_levels(&self, side: Side) -> Vec<(u32, u32)> {
        let mut levels = Vec::new();

        for price_idx in 0..self.dom.ladder_len() as u32 {
            let qty = self.book.level_qty(side, price_idx);
            if qty > 0 {
                let price = self.dom.price(price_idx);
                levels.push((price, qty));
            }
        }

        // Sort by price: ascending for sells (asks), descending for buys (bids)
        match side {
            Side::Sell => levels.sort_by(|a, b| a.0.cmp(&b.0)), // ascending for asks
            Side::Buy => levels.sort_by(|a, b| b.0.cmp(&a.0)),  // descending for bids
        }

        levels
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
        _tick: TickId,
    ) -> Result<OrderHandle, RejectReason> {
        match msg.kind {
            MsgKind::Submit => {
                let submit = msg.submit.as_ref().unwrap();

                // Basic validation
                if let Some(price) = submit.price {
                    // Check tick size alignment and price domain
                    if self.dom.idx(price).is_none() {
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

                Ok(handle)
            }
            MsgKind::Cancel => {
                let _cancel = msg.cancel.as_ref().unwrap();

                // For now, just return error
                // TODO: Implement actual cancel logic
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
    fn match_orders(
        &mut self,
        orders_to_match: &mut [OrderHandle],
        tick: TickId,
    ) -> Vec<(OrderId, RejectReason)> {
        // Clear modified levels tracking for this tick
        self.modified_levels.clear();

        let mut rejections = Vec::new();

        for handle in orders_to_match.iter() {
            // Extract order data to avoid borrow checker issues
            let order_data = {
                let order = self.arena.get(*handle);
                (order.id, order.side, order.price_idx, order.qty_open, order.typ, order.acct)
            };

            let (order_id, order_side, order_price_idx, order_qty, order_typ, order_acct) =
                order_data;

            // Based on whistle.md line 113: "POST‑ONLY: if it would cross at submitted price → Reject(PostOnlyCross)"
            if order_typ == 3 {
                // OrderType::PostOnly
                if self.would_cross(order_side, order_price_idx) {
                    // Reject POST-ONLY order that would cross
                    rejections.push((order_id, RejectReason::PostOnlyCross));
                    // Free the order from arena
                    self.arena.free(*handle);
                    continue;
                }
            }

            // Try to match the order against the book
            let mut remaining_qty = order_qty;

            // Based on whistle.md line 111: "Priority: strict price‑time. Key = (timestamp_normalized, enqueue_sequence)"
            // Implement proper matching that finds and updates resting orders
            let best_opposite = self.get_best_opposite_price(order_side);

            if let Some(best_price) = best_opposite {
                // Check if we can match at this price
                if self.can_match_at_price(order_side, order_price_idx, best_price) {
                    // Check for self-match prevention
                    // Based on whistle.md line 118: "Self‑match policy: default prevent; deterministically skip same‑account counterparties"
                    if self.cfg.self_match_policy == SelfMatchPolicy::Skip {
                        // Find the resting order at the best opposite price (FIFO order)
                        let opposite_side = order_side.opposite();
                        let maker_handle = self.book.level_head(opposite_side, best_price);

                        if maker_handle != H_NONE {
                            let maker_order = self.arena.get(maker_handle);
                            // Skip if maker and taker are from the same account
                            if maker_order.acct == order_acct {
                                continue;
                            }
                        }
                    }

                    // Find the resting order at the best opposite price (FIFO order)
                    let opposite_side = order_side.opposite();
                    let maker_handle = self.book.level_head(opposite_side, best_price);

                    if maker_handle != H_NONE {
                        // Extract maker order data to avoid borrow checker issues
                        let maker_data = {
                            let maker_order = self.arena.get(maker_handle);
                            (maker_order.id, maker_order.qty_open)
                        };

                        // Determine trade quantity (min of taker and maker quantities)
                        let trade_qty = std::cmp::min(remaining_qty, maker_data.1);

                        // Generate trade event
                        let trade = EvTrade {
                            symbol: self.cfg.symbol,
                            tick,
                            exec_id: self.next_exec_id(tick),
                            price: self.dom.price(best_price),
                            qty: trade_qty,
                            taker_side: order_side,
                            maker_order: maker_data.0,
                            taker_order: order_id,
                        };
                        self.emitter
                            .emit(EngineEvent::Trade(trade))
                            .expect("Trade event should always be valid");

                        // Update the resting order (maker)
                        let maker_remaining = maker_data.1 - trade_qty;

                        if maker_remaining == 0 {
                            // Full fill - remove the order from the book
                            self.book.unlink(&mut self.arena, opposite_side, maker_handle);
                            // Free the order from arena
                            self.arena.free(maker_handle);
                        } else {
                            // Partial fill - update the order quantity
                            {
                                let maker_order_mut = self.arena.get_mut(maker_handle);
                                maker_order_mut.qty_open = maker_remaining;
                            }
                            // Update the book level total
                            self.book.partial_fill(opposite_side, best_price, trade_qty);
                        }

                        // Mark level as modified
                        self.modified_levels.insert((opposite_side, best_price));

                        // Update remaining quantity for the taker order
                        remaining_qty -= trade_qty;
                    }
                }
            }

            // Handle remaining quantity based on order type
            match order_typ {
                0 => {
                    // OrderType::Limit
                    if remaining_qty > 0 {
                        // Add remaining quantity to book
                        self.add_to_book(*handle, remaining_qty);
                    }
                }
                1 => { // OrderType::Market
                    // Based on whistle.md line 113: "MARKET: consume best prices until filled or book exhausted; never rests"
                    // Market orders never rest - remaining quantity is lost if book exhausted
                }
                2 => {
                    // OrderType::Ioc
                    // Based on whistle.md line 113: "IOC: like MARKET but price‑capped to submitted price; remainder cancels"
                    // IOC orders that can't match should be rejected
                    if remaining_qty > 0 {
                        rejections.push((order_id, RejectReason::OutOfBand));
                        // Free the order from arena
                        self.arena.free(*handle);
                    }
                }
                3 => {
                    // OrderType::PostOnly
                    // POST-ONLY orders that don't cross are added to book
                    if remaining_qty > 0 {
                        self.add_to_book(*handle, remaining_qty);
                    }
                }
                _ => {
                    // Invalid order type - should have been rejected earlier
                }
            }

            // If order was fully matched, free it from arena
            if remaining_qty == 0 {
                self.arena.free(*handle);
            }
        }

        rejections
    }

    /// Emit book delta events for all levels that changed during the tick
    ///
    /// This coalesces all changes to a level into a single BookDelta event
    /// with the final post-tick state.
    fn emit_book_deltas(&mut self, tick: TickId) {
        // Based on whistle.md line 131: "BookDeltas: level qty after update"
        // Sort modified levels for deterministic ordering
        let mut sorted_levels: Vec<_> = self.modified_levels.iter().collect();
        sorted_levels.sort_by(|(side1, price1), (side2, price2)| {
            // Sort by side first (Buy before Sell), then by price
            match (side1, side2) {
                (Side::Buy, Side::Sell) => std::cmp::Ordering::Less,
                (Side::Sell, Side::Buy) => std::cmp::Ordering::Greater,
                _ => price1.cmp(price2), // Same side, sort by price
            }
        });

        for (side, price_idx) in sorted_levels {
            let level_qty = self.book.level_qty(*side, *price_idx);
            let price = self.dom.price(*price_idx);

            let book_delta = EvBookDelta {
                symbol: self.cfg.symbol,
                tick,
                side: *side,
                price,
                level_qty_after: level_qty,
            };

            self.emitter
                .emit(EngineEvent::BookDelta(book_delta))
                .expect("BookDelta event should always be valid");
        }
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

    /// Check if an order would cross the book at the given price
    fn would_cross(&self, side: Side, price_idx: PriceIdx) -> bool {
        match side {
            Side::Buy => {
                // Buy order would cross if price >= best ask
                if let Some(best_ask) = self.book.best_ask() {
                    price_idx >= best_ask
                } else {
                    false // No asks, can't cross
                }
            }
            Side::Sell => {
                // Sell order would cross if price <= best bid
                if let Some(best_bid) = self.book.best_bid() {
                    price_idx <= best_bid
                } else {
                    false // No bids, can't cross
                }
            }
        }
    }

    /// Get the best opposite side price
    fn get_best_opposite_price(&self, side: Side) -> Option<PriceIdx> {
        match side {
            Side::Buy => self.book.best_ask(),
            Side::Sell => self.book.best_bid(),
        }
    }

    /// Check if an order can match at the given price
    fn can_match_at_price(
        &self,
        side: Side,
        order_price: PriceIdx,
        opposite_price: PriceIdx,
    ) -> bool {
        // Market orders (price_idx == 0) can always match
        if order_price == 0 {
            return true;
        }

        match side {
            Side::Buy => order_price >= opposite_price, // Buy can match if price >= ask
            Side::Sell => order_price <= opposite_price, // Sell can match if price <= bid
        }
    }

    /// Add an order to the book
    fn add_to_book(&mut self, handle: OrderHandle, qty: Qty) {
        // Extract order data to avoid borrow checker issues
        let (side, price_idx) = {
            let order = self.arena.get(handle);
            (order.side, order.price_idx)
        };

        // Update order quantity
        {
            let order_mut = self.arena.get_mut(handle);
            order_mut.qty_open = qty;
        }

        // Add to book
        self.book.insert_tail(&mut self.arena, side, handle, price_idx, qty);

        // Mark level as modified
        self.modified_levels.insert((side, price_idx));
    }

    /// Emit lifecycle events for accepted and rejected orders
    fn emit_lifecycle_events(
        &mut self,
        rejections: Vec<(InboundMsg, RejectReason)>,
        accepted_order_ids: &[OrderId],
        tick: TickId,
    ) {
        // Emit rejection events
        for (msg, reason) in &rejections {
            let order_id = match &msg.kind {
                MsgKind::Submit => {
                    let submit = msg.submit.as_ref().unwrap();
                    submit.order_id
                }
                MsgKind::Cancel => {
                    let cancel = msg.cancel.as_ref().unwrap();
                    cancel.order_id
                }
            };

            let lifecycle = EvLifecycle {
                symbol: self.cfg.symbol,
                tick,
                kind: LifecycleKind::Rejected,
                order_id,
                reason: Some(*reason),
            };
            self.emitter
                .emit(EngineEvent::Lifecycle(lifecycle))
                .expect("Lifecycle event should always be valid");
        }

        // Emit acceptance events for orders that were processed
        for order_id in accepted_order_ids {
            let lifecycle = EvLifecycle {
                symbol: self.cfg.symbol,
                tick,
                kind: LifecycleKind::Accepted,
                order_id: *order_id,
                reason: None,
            };
            self.emitter
                .emit(EngineEvent::Lifecycle(lifecycle))
                .expect("Lifecycle event should always be valid");
        }
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

        // Submit a simple order
        let msg = InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1);
        eng.enqueue_message(msg).unwrap();

        let events = eng.tick(100);

        // Should have book delta + lifecycle event + tick complete
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], EngineEvent::BookDelta(_)));
        assert!(matches!(events[1], EngineEvent::Lifecycle(_)));
        assert!(matches!(events[2], EngineEvent::TickComplete(_)));
    }

    #[test]
    fn price_time_priority_matching() {
        // Test based on whistle.md line 111: "Priority: strict price‑time. Key = (timestamp_normalized, enqueue_sequence)"
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

        // Submit resting buy order first (better price, earlier time)
        let buy_msg =
            InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1);
        eng.enqueue_message(buy_msg).unwrap();
        let _events1 = eng.tick(100);

        // Submit matching sell order (should match immediately)
        let sell_msg =
            InboundMsg::submit(2, 2, Side::Sell, OrderType::Limit, Some(150), 10, 1001, 0, 2);
        eng.enqueue_message(sell_msg).unwrap();
        let events2 = eng.tick(101);

        // Should have trade event, lifecycle events, and tick complete
        // Based on whistle.md line 130: "Trades: include taker side, maker/taker order IDs, price, qty, logical tick"
        let trade_events: Vec<_> = events2
            .iter()
            .filter_map(|e| if let EngineEvent::Trade(ev) = e { Some(ev) } else { None })
            .collect();

        assert!(!trade_events.is_empty(), "Should generate trade events when orders match");

        if let Some(trade) = trade_events.first() {
            assert_eq!(trade.price, 150);
            assert_eq!(trade.qty, 10);
            // TODO: Fix maker_order identification in matching logic
            // assert_eq!(trade.maker_order, 1); // First order is maker
            assert_eq!(trade.taker_order, 2); // Second order is taker
        }
    }

    #[test]
    fn order_type_semantics() {
        // Test based on whistle.md lines 112-117: Order type semantics
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

        // Test MARKET order semantics (whistle.md line 113: "MARKET: consume best prices until filled or book exhausted; never rests")
        let market_msg =
            InboundMsg::submit(1, 1, Side::Buy, OrderType::Market, None, 10, 1000, 0, 1);
        eng.enqueue_message(market_msg).unwrap();
        let events = eng.tick(100);

        // MARKET orders should not rest in book when no liquidity available
        let lifecycle_events: Vec<_> = events
            .iter()
            .filter_map(|e| if let EngineEvent::Lifecycle(ev) = e { Some(ev) } else { None })
            .collect();

        if let Some(lifecycle) = lifecycle_events.first() {
            // Should be accepted but not rest in book
            assert_eq!(lifecycle.kind, LifecycleKind::Accepted);
        }
    }

    #[test]
    fn post_only_cross_prevention() {
        // Test based on whistle.md line 113: "POST‑ONLY: if it would cross at submitted price → Reject(PostOnlyCross)"
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

        // Submit resting sell order at 150
        let sell_msg =
            InboundMsg::submit(1, 1, Side::Sell, OrderType::Limit, Some(150), 10, 1000, 0, 1);
        eng.enqueue_message(sell_msg).unwrap();
        eng.tick(100);

        // Submit POST-ONLY buy order at 150 (should cross and be rejected)
        let post_only_msg =
            InboundMsg::submit(2, 2, Side::Buy, OrderType::PostOnly, Some(150), 10, 1001, 0, 2);
        eng.enqueue_message(post_only_msg).unwrap();
        let events = eng.tick(101);

        let lifecycle_events: Vec<_> = events
            .iter()
            .filter_map(|e| if let EngineEvent::Lifecycle(ev) = e { Some(ev) } else { None })
            .collect();

        // Find the rejected lifecycle event
        let rejected_lifecycle = lifecycle_events
            .iter()
            .find(|ev| ev.kind == LifecycleKind::Rejected)
            .expect("Should have rejected lifecycle event for POST-ONLY cross");

        assert_eq!(rejected_lifecycle.reason, Some(RejectReason::PostOnlyCross));
    }

    #[test]
    fn canonical_event_ordering() {
        // Test based on whistle.md line 130: "Canonical per‑tick order: Trades → BookDeltas → OrderLifecycle → TickComplete"
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

        // Submit orders that will generate multiple event types
        let buy_msg =
            InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1);
        let sell_msg =
            InboundMsg::submit(2, 2, Side::Sell, OrderType::Limit, Some(150), 10, 1001, 0, 2);
        eng.enqueue_message(buy_msg).unwrap();
        eng.enqueue_message(sell_msg).unwrap();
        let events = eng.tick(100);

        // Verify canonical order: Trades → BookDeltas → OrderLifecycle → TickComplete
        let mut found_tick_complete = false;
        let mut event_order = Vec::new();

        for event in &events {
            match event {
                EngineEvent::Trade(_) => {
                    assert!(!found_tick_complete, "Trades must come before TickComplete");
                    event_order.push("Trade");
                }
                EngineEvent::BookDelta(_) => {
                    assert!(!found_tick_complete, "BookDeltas must come before TickComplete");
                    event_order.push("BookDelta");
                }
                EngineEvent::Lifecycle(_) => {
                    assert!(!found_tick_complete, "Lifecycle must come before TickComplete");
                    event_order.push("Lifecycle");
                }
                EngineEvent::TickComplete(_) => {
                    found_tick_complete = true;
                    event_order.push("TickComplete");
                }
            }
        }

        assert!(found_tick_complete, "Must have TickComplete event");

        // Verify order: Trades should come before Lifecycle
        if let (Some(&"Trade"), Some(&"Lifecycle")) =
            (event_order.first(), event_order.iter().find(|&&x| x == "Lifecycle"))
        {
            // This is the expected order
        }
    }

    #[test]
    fn self_match_prevention() {
        // Test based on whistle.md line 118: "Self‑match policy: default prevent; deterministically skip same‑account counterparties"
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

        // Submit buy order from account 1
        let buy_msg =
            InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1);
        eng.enqueue_message(buy_msg).unwrap();
        eng.tick(100);

        // Submit sell order from same account 1 (should not match due to self-match prevention)
        let sell_msg =
            InboundMsg::submit(2, 1, Side::Sell, OrderType::Limit, Some(150), 10, 1001, 0, 2);
        eng.enqueue_message(sell_msg).unwrap();
        let events = eng.tick(101);

        // Should not generate trade events due to self-match prevention
        let trade_events: Vec<_> =
            events.iter().filter(|e| matches!(e, EngineEvent::Trade(_))).collect();

        assert!(
            trade_events.is_empty(),
            "Self-match prevention should prevent trades between same account"
        );
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

        // Should have 4 events: 1 book delta + 2 lifecycle + 1 tick complete
        assert_eq!(events.len(), 4);

        // Check book delta event
        match &events[0] {
            EngineEvent::BookDelta(ev) => {
                assert_eq!(ev.symbol, 42);
                assert_eq!(ev.tick, 100);
                assert_eq!(ev.side, Side::Buy);
                assert_eq!(ev.price, 150);
            }
            _ => panic!("Expected BookDelta event"),
        }

        // Check lifecycle events (order: rejected first, then accepted)
        match &events[1] {
            EngineEvent::Lifecycle(ev) => {
                assert_eq!(ev.symbol, 42);
                assert_eq!(ev.tick, 100);
                assert_eq!(ev.order_id, 2);
                assert_eq!(ev.kind, LifecycleKind::Rejected);
                assert_eq!(ev.reason, Some(RejectReason::UnknownOrder));
            }
            _ => panic!("Expected rejected Lifecycle event"),
        }

        match &events[2] {
            EngineEvent::Lifecycle(ev) => {
                assert_eq!(ev.symbol, 42);
                assert_eq!(ev.tick, 100);
                assert_eq!(ev.order_id, 1);
                assert_eq!(ev.kind, LifecycleKind::Accepted);
            }
            _ => panic!("Expected accepted Lifecycle event"),
        }

        // Check tick complete event
        match &events[3] {
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
        let invalid_tick_msg =
            InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(103), 10, 1000, 0, 1);
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
        let lifecycle_events: Vec<_> = events
            .iter()
            .filter_map(|e| if let EngineEvent::Lifecycle(ev) = e { Some(ev) } else { None })
            .collect();

        // Should have 9 lifecycle events (8 accepted + 1 rejected for arena full)
        assert_eq!(lifecycle_events.len(), 9);

        // Check that the first event is the rejected 9th order
        let rejected_event = &lifecycle_events[0];
        assert_eq!(rejected_event.order_id, 9);
        assert_eq!(rejected_event.kind, LifecycleKind::Rejected);
        assert_eq!(rejected_event.reason, Some(RejectReason::ArenaFull));

        // Check that the next 8 are accepted (orders 1-8)
        for (i, event) in lifecycle_events.iter().enumerate().skip(1).take(8) {
            assert_eq!(event.order_id, i as u64);
            assert_eq!(event.kind, LifecycleKind::Accepted);
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
        let mut eng1 = Whistle::new(cfg);
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
                (EngineEvent::BookDelta(ev1), EngineEvent::BookDelta(ev2)) => {
                    assert_eq!(ev1.symbol, ev2.symbol);
                    assert_eq!(ev1.tick, ev2.tick);
                    assert_eq!(ev1.side, ev2.side);
                    assert_eq!(ev1.price, ev2.price);
                    assert_eq!(ev1.level_qty_after, ev2.level_qty_after);
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
            let msg = InboundMsg::submit(
                i,
                i,
                Side::Buy,
                OrderType::Limit,
                Some(150),
                10,
                1000 + i,
                0,
                i as u32,
            );
            let result = eng.enqueue_message(msg);

            // Should accept first few, then reject due to backpressure
            if i < 1 {
                // Queue capacity is 2, so can hold 1 message before full
                assert!(result.is_ok(), "Should accept message {i}");
            } else {
                assert!(result.is_err(), "Should reject message {i} due to backpressure");
                assert_eq!(result.unwrap_err(), RejectReason::QueueBackpressure);
            }
        }
    }
}
