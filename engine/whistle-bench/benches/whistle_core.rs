#![allow(dead_code)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use whistle::{
    BandMode, Bands, EngineCfg, EngineEvent, EvTickComplete, EventEmitter, ExecIdMode, InboundMsg,
    OrderType, PriceDomain, ReferencePriceSource, SelfMatchPolicy, Side, Whistle,
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

// NEW: Event emission latency benchmarks
fn bench_event_emission(c: &mut Criterion) {
    let mut emitter = EventEmitter::new(1);

    c.bench_function("event_emission_tick_complete", |b| {
        b.iter(|| {
            emitter.start_tick(black_box(100));
            let tick_complete = EvTickComplete { symbol: 1, tick: 100 };
            emitter.emit(EngineEvent::TickComplete(tick_complete)).unwrap();
            black_box(());
        })
    });
}

fn bench_event_emission_sequence(c: &mut Criterion) {
    let mut emitter = EventEmitter::new(1);

    c.bench_function("event_emission_canonical_sequence", |b| {
        b.iter(|| {
            emitter.start_tick(black_box(100));

            // Emit a trade (simulated)
            let trade = whistle::EvTrade {
                symbol: 1,
                tick: 100,
                exec_id: 12345,
                price: 150,
                qty: 10,
                taker_side: Side::Buy,
                maker_order: 1,
                taker_order: 2,
            };
            emitter.emit(EngineEvent::Trade(trade)).unwrap();

            // Emit tick complete
            let tick_complete = EvTickComplete { symbol: 1, tick: 100 };
            emitter.emit(EngineEvent::TickComplete(tick_complete)).unwrap();
            black_box(());
        })
    });
}

// NEW: Message creation and priority key benchmarks
fn bench_message_creation(c: &mut Criterion) {
    c.bench_function("message_creation_submit", |b| {
        b.iter(|| {
            black_box(InboundMsg::submit(
                123,
                456,
                Side::Buy,
                OrderType::Limit,
                Some(150),
                10,
                1000,
                0,
                1,
            ));
        })
    });
}

fn bench_priority_key_computation(c: &mut Criterion) {
    let msg = InboundMsg::submit(123, 456, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1);

    c.bench_function("priority_key_computation", |b| {
        b.iter(|| {
            black_box(msg.priority_key());
        })
    });
}

// NEW: Execution ID generation benchmark
fn bench_execution_id_generation(c: &mut Criterion) {
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
    let mut engine = Whistle::new(cfg);

    c.bench_function("execution_id_generation", |b| {
        b.iter(|| {
            // Simulate multiple execution IDs per tick
            for _ in 0..100 {
                black_box(engine.next_exec_id(100));
            }
        })
    });
}

// NEW: Basic tick processing benchmark
fn bench_basic_tick_processing(c: &mut Criterion) {
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
    let mut engine = Whistle::new(cfg);

    c.bench_function("basic_tick_processing", |b| {
        b.iter(|| {
            black_box(engine.tick(black_box(100)));
        })
    });
}

criterion_group!(
    benches,
    bench_price_domain_roundtrip,
    bench_price_idx_random,
    bench_event_emission,
    bench_event_emission_sequence,
    bench_message_creation,
    bench_priority_key_computation,
    bench_execution_id_generation,
    bench_basic_tick_processing
);
criterion_main!(benches);
