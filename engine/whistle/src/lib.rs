// Whistle - per-symbol deterministic matching engine

mod price_domain;
mod config;
mod types;
mod arena;
mod order_index;

pub use price_domain::{Price, PriceDomain, PriceIdx};
pub use config::{
    BandMode, Bands, SelfMatchPolicy, ExecIdMode, ReferencePriceSource, EngineCfg
};
pub use types::{OrderId, AccountId, Qty, TsNorm, EnqSeq, Side, OrderType, OrderHandle, H_NONE};
pub use arena::{Arena, Order};
pub use order_index::OrderIndex;

pub type TickId     = u64;

pub struct Whistle {
    cfg: EngineCfg,
    dom: PriceDomain,
}

impl Whistle {
    pub fn new(cfg: EngineCfg) -> Self {
        cfg.validate().expect("invalid EngineCfg");
        let dom = cfg.price_domain;
        Self { cfg, dom }
    }
    #[inline] pub fn tick(&mut self, _t: TickId) {}
    #[inline] pub fn price_domain(&self) -> &PriceDomain { &self.dom }
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
}