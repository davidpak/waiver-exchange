use criterion::{black_box, criterion_group, criterion_main, Criterion};
use order_router::{InboundMsgWithSymbol, OrderRouter, RouterConfig};
use whistle::{InboundMsg, OrderType, Side};

fn bench_route_hot_symbol(c: &mut Criterion) {
    let config = RouterConfig::default();
    let mut router = OrderRouter::new(config);

    // Pre-activate symbol by routing one message
    let warmup_msg = create_test_message(1, 999);
    router.route(100, warmup_msg).unwrap();

    let mut order_id = 1000;

    c.bench_function("route_hot_symbol", |b| {
        b.iter(|| {
            let msg = create_test_message(1, order_id);
            order_id += 1;

            let result = router.route(black_box(100), black_box(msg));
            black_box(result).unwrap();
        })
    });
}

fn bench_route_multi_symbol(c: &mut Criterion) {
    let config = RouterConfig { num_shards: 4, ..Default::default() };
    let mut router = OrderRouter::new(config);

    // Pre-activate multiple symbols
    for symbol_id in 1..=8 {
        let warmup_msg = create_test_message(symbol_id, 999);
        router.route(100, warmup_msg).unwrap();
    }

    let mut order_id = 1000;

    c.bench_function("route_multi_symbol", |b| {
        b.iter(|| {
            let symbol_id = (order_id % 8) + 1;
            let msg = create_test_message(symbol_id, order_id as u64);
            order_id += 1;

            let result = router.route(black_box(100), black_box(msg));
            black_box(result).unwrap();
        })
    });
}

fn create_test_message(symbol_id: u32, order_id: u64) -> InboundMsgWithSymbol {
    InboundMsgWithSymbol {
        symbol_id,
        msg: InboundMsg::submit(
            order_id,
            1,
            Side::Buy,
            OrderType::Limit,
            Some(150),
            10,
            1000,
            0,
            symbol_id,
        ),
    }
}

criterion_group!(benches, bench_route_hot_symbol, bench_route_multi_symbol);
criterion_main!(benches);
