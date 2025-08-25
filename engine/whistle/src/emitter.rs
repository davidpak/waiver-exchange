// engine/whistle/src/emitter.rs
#![allow(dead_code)]

use crate::{EngineEvent, EventKind, TickId};

/// Event emitter that enforces canonical event ordering
/// Per tick: Trades → BookDeltas → OrderLifecycle → TickComplete
pub struct EventEmitter {
    symbol: u32,
    current_tick: TickId,
    last_event_kind: Option<EventKind>,
    events: Vec<EngineEvent>,
}

impl EventEmitter {
    pub fn new(symbol: u32) -> Self {
        Self { symbol, current_tick: 0, last_event_kind: None, events: Vec::new() }
    }

    /// Start a new tick - reset sequence and last event kind
    pub fn start_tick(&mut self, tick: TickId) {
        self.current_tick = tick;
        self.last_event_kind = None;
        self.events.clear();
    }

    /// Emit an event, enforcing canonical order
    pub fn emit(&mut self, event: EngineEvent) -> Result<(), EmitError> {
        // Verify event belongs to current tick
        if event.tick() != self.current_tick {
            return Err(EmitError::WrongTick { expected: self.current_tick, got: event.tick() });
        }

        // Verify event belongs to this symbol
        if event.symbol() != self.symbol {
            return Err(EmitError::WrongSymbol { expected: self.symbol, got: event.symbol() });
        }

        // Enforce canonical event order
        let event_kind = event.kind();
        if !event_kind.is_valid_sequence(self.last_event_kind) {
            return Err(EmitError::InvalidOrder {
                last: self.last_event_kind,
                current: event_kind,
            });
        }

        // Update last event kind and store event
        self.last_event_kind = Some(event_kind);
        self.events.push(event);
        Ok(())
    }

    /// Get all events for the current tick
    pub fn events(&self) -> &[EngineEvent] {
        &self.events
    }

    /// Take all events for the current tick (consumes the emitter's events)
    pub fn take_events(&mut self) -> Vec<EngineEvent> {
        std::mem::take(&mut self.events)
    }

    /// Check if tick is complete (has TickComplete event)
    pub fn is_tick_complete(&self) -> bool {
        self.last_event_kind == Some(EventKind::TickComplete)
    }

    /// Get current tick ID
    pub fn current_tick(&self) -> TickId {
        self.current_tick
    }

    /// Get symbol ID
    pub fn symbol(&self) -> u32 {
        self.symbol
    }
}

/// Errors that can occur during event emission
#[derive(Debug, thiserror::Error)]
pub enum EmitError {
    #[error("Event has wrong tick: expected {expected}, got {got}")]
    WrongTick { expected: TickId, got: TickId },

    #[error("Event has wrong symbol: expected {expected}, got {got}")]
    WrongSymbol { expected: u32, got: u32 },

    #[error("Invalid event order: last was {:?}, current is {:?}", last, current)]
    InvalidOrder { last: Option<EventKind>, current: EventKind },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EvTickComplete, EvTrade, Side};

    #[test]
    fn basic_emission() {
        let mut emitter = EventEmitter::new(1);
        emitter.start_tick(100);

        // Emit a trade
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
        assert!(emitter.emit(EngineEvent::Trade(trade)).is_ok());

        // Emit tick complete
        let tick_complete = EvTickComplete { symbol: 1, tick: 100 };
        assert!(emitter.emit(EngineEvent::TickComplete(tick_complete)).is_ok());

        assert!(emitter.is_tick_complete());
        assert_eq!(emitter.events().len(), 2);
    }

    #[test]
    fn wrong_tick_rejected() {
        let mut emitter = EventEmitter::new(1);
        emitter.start_tick(100);

        let trade = EvTrade {
            symbol: 1,
            tick: 101, // Wrong tick!
            exec_id: 12345,
            price: 150,
            qty: 10,
            taker_side: Side::Buy,
            maker_order: 1,
            taker_order: 2,
        };

        match emitter.emit(EngineEvent::Trade(trade)) {
            Err(EmitError::WrongTick { expected: 100, got: 101 }) => {}
            _ => panic!("Expected WrongTick error"),
        }
    }

    #[test]
    fn wrong_symbol_rejected() {
        let mut emitter = EventEmitter::new(1);
        emitter.start_tick(100);

        let trade = EvTrade {
            symbol: 2, // Wrong symbol!
            tick: 100,
            exec_id: 12345,
            price: 150,
            qty: 10,
            taker_side: Side::Buy,
            maker_order: 1,
            taker_order: 2,
        };

        match emitter.emit(EngineEvent::Trade(trade)) {
            Err(EmitError::WrongSymbol { expected: 1, got: 2 }) => {}
            _ => panic!("Expected WrongSymbol error"),
        }
    }

    #[test]
    fn invalid_order_rejected() {
        let mut emitter = EventEmitter::new(1);
        emitter.start_tick(100);

        // Emit a trade first
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
        assert!(emitter.emit(EngineEvent::Trade(trade)).is_ok());

        // Now try to emit another trade after tick complete (invalid)
        let tick_complete = EvTickComplete { symbol: 1, tick: 100 };
        assert!(emitter.emit(EngineEvent::TickComplete(tick_complete)).is_ok());

        // Try to emit another trade after tick complete - this should fail
        let trade2 = EvTrade {
            symbol: 1,
            tick: 100,
            exec_id: 12346,
            price: 150,
            qty: 10,
            taker_side: Side::Sell,
            maker_order: 3,
            taker_order: 4,
        };
        match emitter.emit(EngineEvent::Trade(trade2)) {
            Err(EmitError::InvalidOrder {
                last: Some(EventKind::TickComplete),
                current: EventKind::Trade,
            }) => {}
            _ => panic!("Expected InvalidOrder error"),
        }
    }

    #[test]
    fn canonical_order_enforced() {
        let mut emitter = EventEmitter::new(1);
        emitter.start_tick(100);

        // Valid sequence: Trade -> TickComplete
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
        assert!(emitter.emit(EngineEvent::Trade(trade)).is_ok());

        let tick_complete = EvTickComplete { symbol: 1, tick: 100 };
        assert!(emitter.emit(EngineEvent::TickComplete(tick_complete)).is_ok());

        // Invalid: try to emit another trade after tick complete
        let trade2 = EvTrade {
            symbol: 1,
            tick: 100,
            exec_id: 12346,
            price: 150,
            qty: 10,
            taker_side: Side::Sell,
            maker_order: 3,
            taker_order: 4,
        };
        match emitter.emit(EngineEvent::Trade(trade2)) {
            Err(EmitError::InvalidOrder {
                last: Some(EventKind::TickComplete),
                current: EventKind::Trade,
            }) => {}
            _ => panic!("Expected InvalidOrder error"),
        }
    }
}
