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
pub use types::{AccountId, EnqSeq, H_NONE, OrderHandle, OrderId, OrderType, Qty, Side, TsNorm};

pub type TickId = u64;

pub struct Whistle {
    cfg: EngineCfg,
    dom: PriceDomain,
    emitter: EventEmitter,
    seq_in_tick: u32, // Sequence number for events within the current tick
}

impl Whistle {
    pub fn new(cfg: EngineCfg) -> Self {
        cfg.validate().expect("invalid EngineCfg");
        let dom = cfg.price_domain;
        let emitter = EventEmitter::new(cfg.symbol);
        Self { cfg, dom, emitter, seq_in_tick: 0 }
    }

    /// Process a tick - the core matching engine entry point
    /// This is where all order processing and matching occurs
    pub fn tick(&mut self, t: TickId) -> Vec<EngineEvent> {
        // Start new tick - reset sequence counter and emitter
        self.seq_in_tick = 0;
        self.emitter.start_tick(t);

        // TODO: Drain inbound queue (up to batch_max)
        // TODO: Validate & admit orders
        // TODO: Match orders using price-time priority
        // TODO: Emit trade events
        // TODO: Emit book delta events
        // TODO: Emit lifecycle events

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

        // Process a tick
        let events = eng.tick(100);

        // Should emit exactly one TickComplete event
        assert_eq!(events.len(), 1);
        match &events[0] {
            EngineEvent::TickComplete(ev) => {
                assert_eq!(ev.symbol, 42);
                assert_eq!(ev.tick, 100);
            }
            _ => panic!("Expected TickComplete event"),
        }
    }
}
