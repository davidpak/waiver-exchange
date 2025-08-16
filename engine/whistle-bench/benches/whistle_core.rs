use criterion::{Criterion, black_box, criterion_group, criterion_main};
use whistle::{
    BandMode, Bands, EngineCfg, ExecIdMode, PriceDomain, ReferencePriceSource, SelfMatchPolicy,
    Whistle,
};

fn bench_price_domain_roundtrip(c: &mut Criterion) {
    let cfg = EngineCfg {
        symbol: 1,
        price_domain: PriceDomain { floor: 1_000, ceil: 2_000, tick: 5 },
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

    let engine = Whistle::new(cfg);
    let dom = engine.price_domain();

    c.bench_function("price_idx_roundtrip", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for p in (dom.floor..dom.ceil).step_by(dom.tick as usize) {
                let i = black_box(dom.idx(p).unwrap());
                acc ^= black_box(dom.price(i));
            }
            acc
        })
    });
}

fn bench_price_idx_random(c: &mut Criterion) {
    let cfg = EngineCfg {
        symbol: 1,
        price_domain: PriceDomain { floor: 1_000, ceil: 2_000, tick: 5 },
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
    let engine = Whistle::new(cfg);
    let dom = engine.price_domain();
    let prices: Vec<u32> = (dom.floor..=dom.ceil).step_by(dom.tick as usize).collect();

    c.bench_function("price_idx_random", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            let mut s: u64 = 0xDEAD_BEEF_CAFE_BABE; // fixed seed → deterministic
            // Do a fixed amount of work per iteration
            for _ in 0..1024 {
                // simple LCG step
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
                let i = (s as usize) % prices.len();
                let p = prices[i];

                let idx = black_box(dom.idx(p).unwrap());
                acc ^= black_box(dom.price(idx));
            }
            acc
        })
    });
}

criterion_group!(benches, bench_price_domain_roundtrip);
criterion_main!(benches);
