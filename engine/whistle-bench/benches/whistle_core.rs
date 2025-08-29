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

// NEW: Order matching performance benchmark
fn bench_order_matching_performance(c: &mut Criterion) {
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

    c.bench_function("order_matching_limit_orders", |b| {
        b.iter(|| {
            // Submit a limit order
            let msg =
                InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1);
            engine.enqueue_message(msg).unwrap();

            // Process the tick
            let events = engine.tick(100);
            black_box(events);
        })
    });

    c.bench_function("order_matching_market_orders", |b| {
        b.iter(|| {
            // First add some liquidity
            let msg1 =
                InboundMsg::submit(1, 1, Side::Sell, OrderType::Limit, Some(155), 10, 1000, 0, 1);
            engine.enqueue_message(msg1).unwrap();
            engine.tick(100);

            // Then submit a market order
            let msg2 = InboundMsg::submit(2, 2, Side::Buy, OrderType::Market, None, 5, 1000, 0, 1);
            engine.enqueue_message(msg2).unwrap();

            // Process the tick
            let events = engine.tick(101);
            black_box(events);
        })
    });

    c.bench_function("order_matching_ioc_orders", |b| {
        b.iter(|| {
            // First add some liquidity
            let msg1 =
                InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(160), 10, 1000, 0, 1);
            engine.enqueue_message(msg1).unwrap();
            engine.tick(100);

            // Then submit an IOC order that should match
            let msg2 =
                InboundMsg::submit(2, 2, Side::Sell, OrderType::Ioc, Some(160), 5, 1000, 0, 1);
            engine.enqueue_message(msg2).unwrap();

            // Process the tick
            let events = engine.tick(101);
            black_box(events);
        })
    });
}

// NEW: Realistic throughput benchmark
fn bench_throughput_performance(c: &mut Criterion) {
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

    c.bench_function("throughput_limit_orders_100", |b| {
        b.iter(|| {
            let mut engine = Whistle::new(cfg);
            let tick = 100;

            // Submit 100 limit orders
            for i in 0..100 {
                let msg = InboundMsg::submit(
                    i + 1,
                    (i % 10) + 1, // 10 different accounts
                    if i % 2 == 0 { Side::Buy } else { Side::Sell },
                    OrderType::Limit,
                    Some((150 + (i % 20) * 5) as u32), // Prices from 150 to 245
                    10,                                // Fixed quantity
                    tick * 1000 + i,
                    0,
                    1,
                );
                engine.enqueue_message(msg).unwrap();
            }

            // Process the tick
            let events = engine.tick(tick);
            black_box(events);
        })
    });

    c.bench_function("throughput_mixed_orders_100", |b| {
        b.iter(|| {
            let mut engine = Whistle::new(cfg);
            let tick = 100;

            // Submit 100 mixed orders (limit, market, IOC, POST-ONLY)
            for i in 0..100 {
                let order_type = match i % 4 {
                    0 => OrderType::Limit,
                    1 => OrderType::Market,
                    2 => OrderType::Ioc,
                    3 => OrderType::PostOnly,
                    _ => unreachable!(),
                };

                let msg = InboundMsg::submit(
                    i + 1,
                    (i % 10) + 1,
                    if i % 2 == 0 { Side::Buy } else { Side::Sell },
                    order_type,
                    if order_type == OrderType::Market {
                        None
                    } else {
                        Some((150 + (i % 20) * 5) as u32)
                    },
                    10,
                    tick * 1000 + i,
                    0,
                    1,
                );
                engine.enqueue_message(msg).unwrap();
            }

            // Process the tick
            let events = engine.tick(tick);
            black_box(events);
        })
    });

    c.bench_function("throughput_sustained_1000_orders", |b| {
        b.iter(|| {
            let mut engine = Whistle::new(cfg);
            let mut tick = 100;

            // Submit 1000 orders across multiple ticks
            for batch in 0..10 {
                // Submit 100 orders per tick
                for i in 0..100 {
                    let msg = InboundMsg::submit(
                        batch * 100 + i + 1,
                        (i % 10) + 1,
                        if i % 2 == 0 { Side::Buy } else { Side::Sell },
                        OrderType::Limit,
                        Some((150 + (i % 20) * 5) as u32),
                        10,
                        tick * 1000 + i,
                        0,
                        1,
                    );
                    engine.enqueue_message(msg).unwrap();
                }

                // Process the tick
                let events = engine.tick(tick);
                black_box(events);
                tick += 1;
            }
        })
    });
}

// NEW: Realistic 1-second throughput test
fn bench_throughput_1_second(c: &mut Criterion) {
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

    c.bench_function("throughput_1_second_limit_orders", |b| {
        b.iter(|| {
            let mut engine = Whistle::new(cfg);
            let mut tick = 100;
            let mut total_orders = 0;
            let start_time = std::time::Instant::now();

            // Process orders for exactly 1 second
            while start_time.elapsed() < std::time::Duration::from_secs(1) {
                let orders_this_tick = 1000; // Stay under queue capacity

                // Submit orders for this tick
                for i in 0..orders_this_tick {
                    let msg = InboundMsg::submit(
                        total_orders + i + 1,
                        ((total_orders + i) % 100) + 1, // 100 different accounts
                        if (total_orders + i) % 2 == 0 { Side::Buy } else { Side::Sell },
                        OrderType::Limit,
                        Some((150 + ((total_orders + i) % 20) * 5) as u32),
                        10, // Fixed quantity
                        tick * 1000 + i,
                        0,
                        1,
                    );
                    engine.enqueue_message(msg).unwrap();
                }

                // Process the tick
                let events = engine.tick(tick);
                black_box(events);

                total_orders += orders_this_tick;
                tick += 1;
            }

            // Print the actual throughput for this run
            println!("Limit Orders: {total_orders} orders in 1 second = {total_orders} orders/s");

            // Return the total orders processed in 1 second
            total_orders
        })
    });

    c.bench_function("throughput_1_second_mixed_orders", |b| {
        b.iter(|| {
            let mut engine = Whistle::new(cfg);
            let mut tick = 100;
            let mut total_orders = 0;
            let start_time = std::time::Instant::now();

            // First, add some initial liquidity to create matching opportunities
            for i in 0..1000 {
                let msg = InboundMsg::submit(
                    i + 1,
                    (i % 10) + 1,
                    if i % 2 == 0 { Side::Buy } else { Side::Sell },
                    OrderType::Limit,
                    Some((150 + (i % 20) * 5) as u32),
                    10,
                    tick * 1000 + i,
                    0,
                    1,
                );
                engine.enqueue_message(msg).unwrap();
            }
            engine.tick(tick);
            tick += 1;

            // Process mixed orders for exactly 1 second
            while start_time.elapsed() < std::time::Duration::from_secs(1) {
                let orders_this_tick = 1000; // Stay under queue capacity

                // Submit mixed orders for this tick
                for i in 0..orders_this_tick {
                    let order_type = match (total_orders + i) % 4 {
                        0 => OrderType::Limit,
                        1 => OrderType::Market,
                        2 => OrderType::Ioc,
                        3 => OrderType::PostOnly,
                        _ => unreachable!(),
                    };

                    let msg = InboundMsg::submit(
                        total_orders + i + 1001, // Start after initial liquidity
                        ((total_orders + i) % 100) + 1,
                        if (total_orders + i) % 2 == 0 { Side::Buy } else { Side::Sell },
                        order_type,
                        if order_type == OrderType::Market {
                            None
                        } else {
                            Some((150 + ((total_orders + i) % 20) * 5) as u32)
                        },
                        10,
                        tick * 1000 + i,
                        0,
                        1,
                    );
                    engine.enqueue_message(msg).unwrap();
                }

                // Process the tick
                let events = engine.tick(tick);
                black_box(events);

                total_orders += orders_this_tick;
                tick += 1;
            }

            // Print the actual throughput for this run
            println!("Mixed Orders: {total_orders} orders in 1 second = {total_orders} orders/s");

            // Return the total orders processed in 1 second
            total_orders
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
    bench_basic_tick_processing,
    bench_order_matching_performance,
    bench_throughput_performance,
    bench_throughput_1_second
);
criterion_main!(benches);
