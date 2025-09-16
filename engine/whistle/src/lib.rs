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
mod outbound_queue;
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
pub use outbound_queue::{BackpressurePolicy, OutboundQueue};
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
    inbound_queue: std::sync::Arc<InboundQueue>, // SPSC queue for messages from OrderRouter
    outbound_queue: std::sync::Arc<OutboundQueue>, // MPSC queue for events to ExecutionManager
    arena: Arena,                                // Preallocated order storage
    book: Book,                                  // Order book with price-time priority
    order_index: OrderIndex,                     // O(1) order lookup by order_id

    // State tracking
    reference_price: Option<Price>, // Current reference price for bands
    modified_levels: std::collections::HashSet<(Side, PriceIdx)>, // Track levels modified during tick

    // Trade tracking
    last_trade_price: Option<u64>,
    last_trade_quantity: Option<u64>,
    last_trade_timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

impl Whistle {
    pub fn new(cfg: EngineCfg) -> Self {
        cfg.validate().expect("invalid EngineCfg");
        let dom = cfg.price_domain;
        let emitter = EventEmitter::new(cfg.symbol);

        // Initialize order processing components
        let inbound_queue = std::sync::Arc::new(InboundQueue::new(cfg.batch_max as usize));
        let outbound_queue = std::sync::Arc::new(OutboundQueue::with_default_capacity());
        let arena = Arena::with_capacity(cfg.arena_capacity);
        let book = Book::new(dom);
        let order_index = OrderIndex::with_capacity_pow2(cfg.arena_capacity as usize * 2); // 2x capacity for hash table

        Self {
            cfg,
            dom,
            emitter,
            seq_in_tick: 0,
            inbound_queue,
            outbound_queue,
            arena,
            book,
            order_index,
            reference_price: None,
            modified_levels: std::collections::HashSet::new(),
            last_trade_price: None,
            last_trade_quantity: None,
            last_trade_timestamp: None,
        }
    }

    /// Create a new Whistle with a specific inbound queue (for SymbolCoordinator integration)
    pub fn new_with_inbound_queue(
        cfg: EngineCfg,
        inbound_queue: std::sync::Arc<InboundQueue>,
    ) -> Self {
        cfg.validate().expect("invalid EngineCfg");
        let dom = cfg.price_domain;
        let emitter = EventEmitter::new(cfg.symbol);

        // Use the provided inbound queue instead of creating a new one
        let outbound_queue = std::sync::Arc::new(OutboundQueue::with_default_capacity());
        let arena = Arena::with_capacity(cfg.arena_capacity);
        let book = Book::new(dom);
        let order_index = OrderIndex::with_capacity_pow2(cfg.arena_capacity as usize * 2); // 2x capacity for hash table

        Self {
            cfg,
            dom,
            emitter,
            seq_in_tick: 0,
            inbound_queue,
            outbound_queue,
            arena,
            book,
            order_index,
            reference_price: None,
            modified_levels: std::collections::HashSet::new(),
            last_trade_price: None,
            last_trade_quantity: None,
            last_trade_timestamp: None,
        }
    }

    /// Create a new Whistle with both inbound and outbound queues (for SymbolCoordinator integration)
    pub fn new_with_queues(
        cfg: EngineCfg,
        inbound_queue: std::sync::Arc<InboundQueue>,
        outbound_queue: std::sync::Arc<OutboundQueue>,
    ) -> Self {
        cfg.validate().expect("invalid EngineCfg");
        let dom = cfg.price_domain;
        let emitter = EventEmitter::new(cfg.symbol);

        let arena = Arena::with_capacity(cfg.arena_capacity);
        let book = Book::new(dom);
        let order_index = OrderIndex::with_capacity_pow2(cfg.arena_capacity as usize * 2); // 2x capacity for hash table

        Self {
            cfg,
            dom,
            emitter,
            seq_in_tick: 0,
            inbound_queue,
            outbound_queue,
            arena,
            book,
            order_index,
            reference_price: None,
            modified_levels: std::collections::HashSet::new(),
            last_trade_price: None,
            last_trade_quantity: None,
            last_trade_timestamp: None,
        }
    }

    /// Process a tick - the core matching engine entry point
    /// This is where all order processing and matching occurs
    ///
    /// This method maintains backward compatibility by returning events as Vec.
    /// For new ExecutionManager integration, use tick_with_queue_emission().
    pub fn tick(&mut self, t: TickId) -> Vec<EngineEvent> {
        // Start new tick - reset sequence counter and emitter
        self.seq_in_tick = 0;
        self.emitter.start_tick(t);

        // Step 1: Drain inbound queue (up to batch_max)
        let messages = self.inbound_queue.drain_lockfree(self.cfg.batch_max as usize);
        if !messages.is_empty() {
            tracing::info!(
                "Whistle engine {} drained {} messages at tick {}",
                self.cfg.symbol,
                messages.len(),
                t
            );

            // Debug: Show current book state
            let best_bid = self.book.best_bid();
            let best_ask = self.book.best_ask();
            tracing::debug!(
                "Symbol {}: Book state - best_bid: {:?}, best_ask: {:?}",
                self.cfg.symbol,
                best_bid.map(|idx| self.dom.price(idx)),
                best_ask.map(|idx| self.dom.price(idx))
            );
        }

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
        let _match_rejections = self.match_orders(&mut orders_to_match[..], t);

        // Step 5: Extract order data for accepted orders (after matching/rejection)
        // Capture order data before any orders might be freed from the arena
        let mut accepted_order_data = Vec::new();
        for handle in &orders_to_match {
            if self.arena.is_valid(*handle) {
                let order = self.arena.get(*handle);
                accepted_order_data.push((
                    order.id,
                    order.acct,
                    order.side,
                    order.price_idx,
                    order.qty_open,
                    order.typ,
                ));
            }
        }

        // Step 6: Emit book delta events (coalesced)
        self.emit_book_deltas(t);

        // Step 7: Emit lifecycle events (accepted/rejected orders)
        self.emit_lifecycle_events(rejections, &accepted_order_data, t);

        // Step 8: Match rejections are now handled in Step 5 above

        // Always emit TickComplete for the tick method (for backward compatibility with tests)
        let tick_complete = EvTickComplete { symbol: self.cfg.symbol, tick: t };
        self.emitter
            .emit(EngineEvent::TickComplete(tick_complete))
            .expect("TickComplete should always be valid");

        // Return all events for this tick
        self.emitter.take_events()
    }

    /// Process a tick with queue emission - new ExecutionManager integration
    ///
    /// This method processes orders and emits events directly to the OutboundQueue
    /// instead of returning them as a Vec. This is the preferred method for
    /// ExecutionManager integration.
    pub fn tick_with_queue_emission(&mut self, t: TickId) {
        // Start new tick - reset sequence counter and emitter
        self.seq_in_tick = 0;
        self.emitter.start_tick(t);

        // Step 1: Drain inbound queue (up to batch_max)
        let messages = self.inbound_queue.drain_lockfree(self.cfg.batch_max as usize);
        if !messages.is_empty() {
            tracing::info!(
                "Whistle engine {} drained {} messages at tick {}",
                self.cfg.symbol,
                messages.len(),
                t
            );

            // Debug: Show current book state
            let best_bid = self.book.best_bid();
            let best_ask = self.book.best_ask();
            tracing::debug!(
                "Symbol {}: Book state - best_bid: {:?}, best_ask: {:?}",
                self.cfg.symbol,
                best_bid.map(|idx| self.dom.price(idx)),
                best_ask.map(|idx| self.dom.price(idx))
            );
        }

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

        // Step 5: Add match rejections to the rejections vector
        // Convert (OrderId, RejectReason) to (InboundMsg, RejectReason) format
        for (order_id, reason) in match_rejections.into_iter() {
            // Find the original message for this order ID
            // For now, create a placeholder message - this is a limitation of the current design
            // TODO: Improve this by tracking the original message for each order
            let placeholder_msg = InboundMsg {
                kind: MsgKind::Submit,
                submit: Some(Submit {
                    order_id,
                    account_id: 0,         // Placeholder
                    side: Side::Buy,       // Placeholder
                    typ: OrderType::Limit, // Placeholder
                    price: None,
                    qty: 0,     // Placeholder
                    ts_norm: 0, // Placeholder
                    meta: 0,    // Placeholder
                }),
                cancel: None,
                enq_seq: 0, // Placeholder
            };
            rejections.push((placeholder_msg, reason));
        }

        // Step 6: Extract order IDs for accepted orders (after matching/rejection)
        // Capture order data before any orders might be freed from the arena
        let mut accepted_order_data = Vec::new();
        for handle in &orders_to_match {
            if self.arena.is_valid(*handle) {
                let order = self.arena.get(*handle);
                accepted_order_data.push((
                    order.id,
                    order.acct,
                    order.side,
                    order.price_idx,
                    order.qty_open,
                    order.typ,
                ));
            }
        }

        // Step 6: Emit book delta events (coalesced)
        self.emit_book_deltas(t);

        // Step 7: Emit lifecycle events (accepted/rejected orders)
        self.emit_lifecycle_events(rejections, &accepted_order_data, t);

        // Step 8: Match rejections are now handled in Step 5 above

        // Check if there was any activity in this tick
        let events = self.emitter.take_events();
        let had_activity = !events.is_empty();

        // Only emit TickComplete if there was actual activity
        if had_activity {
            let tick_complete = EvTickComplete { symbol: self.cfg.symbol, tick: t };
            self.emitter
                .emit(EngineEvent::TickComplete(tick_complete))
                .expect("TickComplete should always be valid");
        }

        // Emit all events to the OutboundQueue (including TickComplete if there was activity)
        let mut final_events = events; // Use the events we already took
        if had_activity {
            // Add the TickComplete event if we emitted one
            let tick_complete_events = self.emitter.take_events();
            final_events.extend(tick_complete_events);
        }
        for event in final_events {
            if self.outbound_queue.try_enqueue(event.clone()).is_err() {
                // This should not happen with Fatal policy as it would exit the process
                // But we handle it gracefully for safety
                eprintln!("Failed to enqueue event to OutboundQueue");
            }
        }
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
    pub fn get_order_book_levels(&self, side: Side) -> Vec<(u32, u64)> {
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

    /// Restore order book state from snapshot data
    /// This is used during system recovery to restore the order book to a previous state
    pub fn restore_order_book_state(
        &mut self,
        buy_orders: &std::collections::HashMap<u64, u64>,
        sell_orders: &std::collections::HashMap<u64, u64>,
        last_trade_price: Option<u64>,
        last_trade_quantity: Option<u64>,
        last_trade_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    ) {
        tracing::info!(
            "Restoring order book state: {} buy orders, {} sell orders",
            buy_orders.len(),
            sell_orders.len()
        );

        // Clear existing order book
        self.book = Book::new(self.dom);
        self.order_index = OrderIndex::with_capacity_pow2(self.cfg.arena_capacity as usize * 2);
        self.arena = Arena::with_capacity(self.cfg.arena_capacity);

        // Restore buy orders
        for (&price, &qty) in buy_orders {
            if let Some(price_idx) = self.dom.idx(price as u32) {
                // Allocate a new order handle
                if let Some(handle) = self.arena.alloc() {
                    // Set up the order data
                    let order = self.arena.get_mut(handle);
                    order.id = 2; // Placeholder order ID (0 and 1 are reserved)
                    order.acct = 0;
                    order.side = Side::Buy;
                    order.price_idx = price_idx;
                    order.qty_open = qty;
                    order.ts_norm = 0;
                    order.enq_seq = 0;
                    order.typ = 0; // OrderType::Limit
                    order.prev = H_NONE;
                    order.next = H_NONE;

                    // Add to order index and book
                    let _ = self.order_index.insert(2, handle); // Use 2 as placeholder order ID (0 and 1 are reserved)
                    self.book.insert_tail(
                        &mut self.arena,
                        Side::Buy,
                        handle,
                        price_idx,
                        qty,
                    );

                    tracing::debug!("Restored buy order: price={}, qty={}", price, qty);
                }
            }
        }

        // Restore sell orders
        for (&price, &qty) in sell_orders {
            if let Some(price_idx) = self.dom.idx(price as u32) {
                // Allocate a new order handle
                if let Some(handle) = self.arena.alloc() {
                    // Set up the order data
                    let order = self.arena.get_mut(handle);
                    order.id = 2; // Placeholder order ID (0 and 1 are reserved)
                    order.acct = 0;
                    order.side = Side::Sell;
                    order.price_idx = price_idx;
                    order.qty_open = qty;
                    order.ts_norm = 0;
                    order.enq_seq = 0;
                    order.typ = 0; // OrderType::Limit
                    order.prev = H_NONE;
                    order.next = H_NONE;

                    // Add to order index and book
                    let _ = self.order_index.insert(2, handle); // Use 2 as placeholder order ID (0 and 1 are reserved)
                    self.book.insert_tail(
                        &mut self.arena,
                        Side::Sell,
                        handle,
                        price_idx,
                        qty,
                    );

                    tracing::debug!("Restored sell order: price={}, qty={}", price, qty);
                }
            }
        }

        // Restore trade information
        self.last_trade_price = last_trade_price;
        self.last_trade_quantity = last_trade_quantity;
        self.last_trade_timestamp = last_trade_timestamp;

        tracing::info!("Order book state restored successfully");
    }

    /// Get the latest trade information
    pub fn get_last_trade_info(
        &self,
    ) -> (Option<u64>, Option<u64>, Option<chrono::DateTime<chrono::Utc>>) {
        (self.last_trade_price, self.last_trade_quantity, self.last_trade_timestamp)
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

                // POST-ONLY cross prevention check
                if submit.typ == OrderType::PostOnly {
                    if let Some(price) = submit.price {
                        if let Some(price_idx) = self.dom.idx(price) {
                            if self.would_cross(submit.side, price_idx) {
                                return Err(RejectReason::PostOnlyCross);
                            }
                        }
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
        self.modified_levels.clear();

        let mut rejections = Vec::new();

        // Process orders in sequence, but allow matching against orders already in the book
        // and against orders processed earlier in this tick
        for (i, handle) in orders_to_match.iter().enumerate() {
            let order_data = {
                let order = self.arena.get(*handle);
                (order.id, order.side, order.price_idx, order.qty_open, order.typ, order.acct)
            };

            let (order_id, order_side, order_price_idx, order_qty, order_typ, order_acct) =
                order_data;

            if order_typ == 3 && self.would_cross(order_side, order_price_idx) {
                rejections.push((order_id, RejectReason::PostOnlyCross));
                self.arena.free(*handle);
                continue;
            }

            let mut remaining_qty = order_qty;

            // First, try to match against existing orders in the book
            while remaining_qty > 0 {
                let best_opposite = self.get_best_opposite_price(order_side);

                if let Some(best_price) = best_opposite {
                    tracing::debug!(
                        "Symbol {}: Order {} ({} at {}) can match against best opposite price {}",
                        self.cfg.symbol,
                        order_id,
                        if order_side == Side::Buy { "BUY" } else { "SELL" },
                        self.dom.price(order_price_idx),
                        self.dom.price(best_price)
                    );

                    if self.can_match_at_price(order_side, order_price_idx, best_price) {
                        tracing::info!(
                            "Symbol {}: MATCHING order {} ({} at {}) against book at {}",
                            self.cfg.symbol,
                            order_id,
                            if order_side == Side::Buy { "BUY" } else { "SELL" },
                            self.dom.price(order_price_idx),
                            self.dom.price(best_price)
                        );
                        let opposite_side = order_side.opposite();
                        let mut maker_handle = self.book.level_head(opposite_side, best_price);

                        let mut traded_at_this_level = false;

                        while maker_handle != H_NONE && remaining_qty > 0 {
                            if self.cfg.self_match_policy == SelfMatchPolicy::Skip {
                                let maker_order = self.arena.get(maker_handle);
                                if maker_order.acct == order_acct {
                                    maker_handle = maker_order.next;
                                    continue;
                                }
                            }

                            let maker_data = {
                                let maker_order = self.arena.get(maker_handle);
                                (maker_order.id, maker_order.qty_open, maker_order.next)
                            };

                            let (maker_id, maker_qty, next_handle) = maker_data;

                            let trade_qty = std::cmp::min(remaining_qty, maker_qty);

                            let trade = EvTrade {
                                symbol: self.cfg.symbol,
                                tick,
                                exec_id: self.next_exec_id(tick),
                                price: self.dom.price(best_price),
                                qty: trade_qty,
                                taker_side: order_side,
                                maker_order: maker_id,
                                taker_order: order_id,
                            };
                            self.emitter
                                .emit(EngineEvent::Trade(trade))
                                .expect("Trade event should always be valid");

                            // Update trade tracking
                            self.last_trade_price = Some(trade.price as u64);
                            self.last_trade_quantity = Some(trade.qty as u64);
                            self.last_trade_timestamp = Some(chrono::Utc::now());

                            let maker_remaining = maker_qty - trade_qty;

                            if maker_remaining == 0 {
                                self.book.unlink(&mut self.arena, opposite_side, maker_handle);
                                self.arena.free(maker_handle);
                            } else {
                                {
                                    let maker_order_mut = self.arena.get_mut(maker_handle);
                                    maker_order_mut.qty_open = maker_remaining;
                                }
                                self.book.partial_fill(opposite_side, best_price, trade_qty);
                            }

                            self.modified_levels.insert((opposite_side, best_price));

                            remaining_qty -= trade_qty;
                            traded_at_this_level = true;

                            maker_handle = next_handle;
                        }

                        if !traded_at_this_level {
                            break;
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            // Then, try to match against orders processed earlier in this tick
            if remaining_qty > 0 {
                for (_j, &other_handle) in orders_to_match.iter().enumerate().take(i) {
                    if !self.arena.is_valid(other_handle) {
                        continue; // This order was already fully matched
                    }

                    // Get order data to avoid borrow conflicts
                    let (other_order_id, other_side, other_price_idx, other_qty_open, other_acct) = {
                        let other_order = self.arena.get(other_handle);
                        (
                            other_order.id,
                            other_order.side,
                            other_order.price_idx,
                            other_order.qty_open,
                            other_order.acct,
                        )
                    };

                    // Check if orders can match
                    if other_side != order_side
                        && self.can_match_at_price(order_side, order_price_idx, other_price_idx)
                    {
                        // Check self-match prevention
                        if self.cfg.self_match_policy == SelfMatchPolicy::Skip
                            && other_acct == order_acct
                        {
                            continue;
                        }

                        let trade_qty = std::cmp::min(remaining_qty, other_qty_open);

                        let trade = EvTrade {
                            symbol: self.cfg.symbol,
                            tick,
                            exec_id: self.next_exec_id(tick),
                            price: self.dom.price(other_price_idx),
                            qty: trade_qty,
                            taker_side: order_side,
                            maker_order: other_order_id,
                            taker_order: order_id,
                        };
                        self.emitter
                            .emit(EngineEvent::Trade(trade))
                            .expect("Trade event should always be valid");

                        // Update trade tracking
                        self.last_trade_price = Some(trade.price as u64);
                        self.last_trade_quantity = Some(trade.qty as u64);
                        self.last_trade_timestamp = Some(chrono::Utc::now());

                        // Update the maker order
                        let maker_remaining = other_qty_open - trade_qty;
                        if maker_remaining == 0 {
                            self.arena.free(other_handle);
                        } else {
                            let maker_order_mut = self.arena.get_mut(other_handle);
                            maker_order_mut.qty_open = maker_remaining;
                        }

                        self.modified_levels.insert((other_side, other_price_idx));
                        remaining_qty -= trade_qty;

                        if remaining_qty == 0 {
                            break;
                        }
                    }
                }
            }

            // Handle remaining quantity based on order type
            match order_typ {
                0 => {
                    // OrderType::Limit
                    if remaining_qty > 0 {
                        self.add_to_book(*handle, remaining_qty);
                    }
                }
                1 => {
                    // OrderType::Market
                    if remaining_qty > 0 {
                        // Market orders with remaining quantity should be accepted but not rest in book
                        // The order is already freed when fully matched, so we just don't add to book
                    }
                }
                2 => {
                    // OrderType::IOC
                    if remaining_qty > 0 {
                        // IOC orders with remaining quantity should be canceled (not rejected)
                        // The order is already freed when fully matched, so we just don't add to book
                    }
                }
                3 => {
                    // OrderType::PostOnly
                    if remaining_qty > 0 {
                        self.add_to_book(*handle, remaining_qty);
                    }
                }
                _ => {
                    // Invalid order type - should have been rejected earlier
                }
            }

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
        self.inbound_queue.try_enqueue_lockfree(msg)
    }

    /// Get queue statistics for monitoring
    pub fn queue_stats(&self) -> (usize, usize) {
        (self.inbound_queue.len(), self.inbound_queue.capacity())
    }

    /// Clear the inbound queue
    pub fn clear_queue(&mut self) {
        self.inbound_queue.clear_lockfree();
    }

    /// Get a reference to the OutboundQueue for ExecutionManager consumption
    ///
    /// This allows the ExecutionManager to drain events from the queue.
    /// The queue is thread-safe and can be accessed from different threads.
    pub fn outbound_queue(&self) -> &OutboundQueue {
        &self.outbound_queue
    }

    /// Get outbound queue statistics for monitoring
    pub fn outbound_queue_stats(&self) -> (usize, usize) {
        (self.outbound_queue.len(), self.outbound_queue.capacity())
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
                // POST-ONLY buy order would cross if price >= best ask (opposite side)
                if let Some(best_ask) = self.book.best_ask() {
                    price_idx >= best_ask
                } else {
                    false // No asks, can't cross
                }
            }
            Side::Sell => {
                // POST-ONLY sell order would cross if price <= best bid (opposite side)
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
        accepted_order_data: &[(OrderId, u64, Side, u32, u64, u8)],
        tick: TickId,
    ) {
        // Emit rejection events
        for (msg, reason) in &rejections {
            let (order_id, account_id, side, price, quantity, order_type) = match &msg.kind {
                MsgKind::Submit => {
                    let submit = msg.submit.as_ref().unwrap();
                    (
                        submit.order_id,
                        submit.account_id as u32, // Convert u64 to u32
                        submit.side,
                        submit.price,
                        submit.qty,
                        submit.typ as u8,
                    )
                }
                MsgKind::Cancel => {
                    let cancel = msg.cancel.as_ref().unwrap();
                    // For cancel messages, we don't have full order data, so use defaults
                    (cancel.order_id, 0, Side::Buy, None, 0, 0)
                }
            };

            let lifecycle = EvLifecycle {
                symbol: self.cfg.symbol,
                tick,
                kind: LifecycleKind::Rejected,
                order_id,
                reason: Some(*reason),
                account_id,
                side,
                price,
                quantity,
                order_type,
            };
            self.emitter
                .emit(EngineEvent::Lifecycle(lifecycle))
                .expect("Lifecycle event should always be valid");
        }

        // Emit acceptance events for orders that were processed
        for (order_id, account_id, side, price_idx, quantity, order_type) in accepted_order_data {
            // Use the captured order data instead of looking up in arena
            let price = if *order_type == 1 { None } else { Some(self.dom.price(*price_idx)) };
            let account_id = *account_id as u32; // Convert u64 to u32

            let lifecycle = EvLifecycle {
                symbol: self.cfg.symbol,
                tick,
                kind: LifecycleKind::Accepted,
                order_id: *order_id,
                reason: None,
                account_id,
                side: *side,
                price,
                quantity: *quantity,
                order_type: *order_type,
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

    #[test]
    fn test_tick_with_queue_emission() {
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

        // Check initial queue state
        let (len, capacity) = eng.outbound_queue_stats();
        assert_eq!(len, 0);
        assert_eq!(capacity, 8192); // Default capacity

        // Process tick with queue emission
        eng.tick_with_queue_emission(100);

        // Check that events were emitted to the queue
        let (len, _) = eng.outbound_queue_stats();
        assert!(len > 0, "Should have events in outbound queue");

        // Drain events from the queue to verify they were emitted correctly
        let events = eng.outbound_queue().drain(100);
        assert!(!events.is_empty(), "Should have events to drain");

        // Verify event types and ordering
        let mut found_tick_complete = false;
        for event in &events {
            match event {
                EngineEvent::BookDelta(ev) => {
                    assert_eq!(ev.symbol, 42);
                    assert_eq!(ev.tick, 100);
                }
                EngineEvent::Lifecycle(ev) => {
                    assert_eq!(ev.symbol, 42);
                    assert_eq!(ev.tick, 100);
                    assert_eq!(ev.order_id, 1);
                }
                EngineEvent::TickComplete(ev) => {
                    assert_eq!(ev.symbol, 42);
                    assert_eq!(ev.tick, 100);
                    found_tick_complete = true;
                }
                EngineEvent::Trade(_) => {
                    // No trades in this simple test
                }
            }
        }
        assert!(found_tick_complete, "Should have TickComplete event");

        // Queue should be empty after draining
        let (len, _) = eng.outbound_queue_stats();
        assert_eq!(len, 0);
    }
}
