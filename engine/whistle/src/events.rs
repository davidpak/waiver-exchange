#![allow(dead_code)]

use crate::{OrderId, Price, Qty, Side, TickId};

/// Trade execution event - emitted when orders match
#[derive(Debug, Clone, Copy)]
pub struct EvTrade {
    pub symbol: u32,
    pub tick: TickId,
    pub exec_id: u64, // (tick << SHIFT) | seq OR assigned centrally
    pub price: Price,
    pub qty: Qty,
    pub taker_side: Side,
    pub maker_order: OrderId,
    pub taker_order: OrderId,
}

/// Book level change event - emitted when level quantities change
#[derive(Debug, Clone, Copy)]
pub struct EvBookDelta {
    pub symbol: u32,
    pub tick: TickId,
    pub side: Side,
    pub price: Price,
    pub level_qty_after: Qty,
}

/// Order lifecycle event - emitted for order state transitions
#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LifecycleKind {
    Accepted = 0,
    Rejected = 1,
    Cancelled = 2,
}

/// Rejection reasons - explicit, enumerable
#[repr(u16)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RejectReason {
    BadTick = 1,
    OutOfBand = 2,
    PostOnlyCross = 3,
    MarketDisallowed = 4, // cold start / halted
    IocDisallowed = 5,    // cold start / halted
    RiskUnavailable = 6,
    InsufficientFunds = 7,
    ExposureExceeded = 8,
    ArenaFull = 9,
    QueueBackpressure = 10,
    Malformed = 11,
    UnknownOrder = 12,     // cancel for non-existent
    SelfMatchBlocked = 13, // if policy=prevent on submit
    MarketHalted = 14,
}

#[derive(Debug, Clone, Copy)]
pub struct EvLifecycle {
    pub symbol: u32,
    pub tick: TickId,
    pub kind: LifecycleKind,
    pub order_id: OrderId,
    pub reason: Option<RejectReason>, // Some(reason) for Rejected, None for Accepted/Cancelled
}

/// Tick completion event - emitted at end of each tick
#[derive(Debug, Clone, Copy)]
pub struct EvTickComplete {
    pub symbol: u32,
    pub tick: TickId,
}

/// Canonical event types emitted by Whistle
/// Order is fixed per tick: Trades → BookDeltas → OrderLifecycle → TickComplete
#[derive(Debug, Clone)]
pub enum EngineEvent {
    Trade(EvTrade),
    BookDelta(EvBookDelta),
    Lifecycle(EvLifecycle),
    TickComplete(EvTickComplete),
}

impl EngineEvent {
    /// Get the event kind for ordering enforcement
    #[inline]
    pub fn kind(&self) -> EventKind {
        match self {
            EngineEvent::Trade(_) => EventKind::Trade,
            EngineEvent::BookDelta(_) => EventKind::BookDelta,
            EngineEvent::Lifecycle(_) => EventKind::Lifecycle,
            EngineEvent::TickComplete(_) => EventKind::TickComplete,
        }
    }

    /// Get the symbol ID for routing
    #[inline]
    pub fn symbol(&self) -> u32 {
        match self {
            EngineEvent::Trade(ev) => ev.symbol,
            EngineEvent::BookDelta(ev) => ev.symbol,
            EngineEvent::Lifecycle(ev) => ev.symbol,
            EngineEvent::TickComplete(ev) => ev.symbol,
        }
    }

    /// Get the tick ID for sequencing
    #[inline]
    pub fn tick(&self) -> TickId {
        match self {
            EngineEvent::Trade(ev) => ev.tick,
            EngineEvent::BookDelta(ev) => ev.tick,
            EngineEvent::Lifecycle(ev) => ev.tick,
            EngineEvent::TickComplete(ev) => ev.tick,
        }
    }
}

/// Event kinds for canonical ordering enforcement
#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum EventKind {
    Trade = 0,
    BookDelta = 1,
    Lifecycle = 2,
    TickComplete = 3,
}

impl EventKind {
    /// Check if event ordering is valid according to canonical sequence
    #[inline]
    pub fn is_valid_sequence(&self, prev: Option<EventKind>) -> bool {
        match prev {
            None => true, // First event in sequence
            Some(prev_kind) => {
                // Canonical order: Trade(0) → BookDelta(1) → Lifecycle(2) → TickComplete(3)
                (*self as u8) >= (prev_kind as u8)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_ordering_enforcement() {
        // Valid sequences
        assert!(EventKind::Trade.is_valid_sequence(None));
        assert!(EventKind::BookDelta.is_valid_sequence(Some(EventKind::Trade)));
        assert!(EventKind::Lifecycle.is_valid_sequence(Some(EventKind::BookDelta)));
        assert!(EventKind::TickComplete.is_valid_sequence(Some(EventKind::Lifecycle)));

        // Invalid sequences
        assert!(!EventKind::Trade.is_valid_sequence(Some(EventKind::BookDelta)));
        assert!(!EventKind::BookDelta.is_valid_sequence(Some(EventKind::Lifecycle)));
        assert!(!EventKind::Lifecycle.is_valid_sequence(Some(EventKind::TickComplete)));

        // Same kind is valid (for multiple events of same type)
        assert!(EventKind::Trade.is_valid_sequence(Some(EventKind::Trade)));
        assert!(EventKind::BookDelta.is_valid_sequence(Some(EventKind::BookDelta)));
    }

    #[test]
    fn event_properties() {
        let trade = EvTrade {
            symbol: 1,
            tick: 100,
            exec_id: 12345,
            price: 150,
            qty: 10,
            taker_side: Side::Buy,
            maker_order: 1,
            taker_order: 2,
        };

        let event = EngineEvent::Trade(trade);
        assert_eq!(event.kind(), EventKind::Trade);
        assert_eq!(event.symbol(), 1);
        assert_eq!(event.tick(), 100);
    }
}
