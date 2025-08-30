use colored::*;
use whistle::{
    BandMode, Bands, EngineCfg, EngineEvent, ExecIdMode, InboundMsg, LifecycleKind, OrderType,
    PriceDomain, ReferencePriceSource, SelfMatchPolicy, Side, Whistle,
};

pub fn run_demo(symbol: u32) {
    println!("{}", "Running Whistle Demo...".green().bold());

    let cfg = EngineCfg {
        symbol,
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

    let mut engine = Whistle::new(cfg);
    let tick = 100;

    // Submit some orders
    let orders = vec![
        (1, Side::Buy, OrderType::Limit, Some(150), 10),
        (2, Side::Sell, OrderType::Limit, Some(155), 10),
        (3, Side::Buy, OrderType::Limit, Some(145), 5),
        (4, Side::Sell, OrderType::Limit, Some(160), 5),
    ];

    for (order_id, side, order_type, price, qty) in orders {
        let msg = InboundMsg::submit(
            order_id as u64,
            1,
            side,
            order_type,
            price,
            qty,
            tick * 1000,
            0, // ts_norm
            0, // enq_seq
        );
        if let Err(e) = engine.enqueue_message(msg) {
            println!("Failed to enqueue order: {:?}", e);
        }
    }

    // Process tick
    let events = engine.tick(tick);

    println!("{}", "Demo Results:".cyan().bold());
    for event in events {
        match event {
            EngineEvent::Trade(trade) => {
                println!(
                    "  💰 Trade: {} @ {} (exec_id: {})",
                    trade.qty, trade.price, trade.exec_id
                );
            }
            EngineEvent::BookDelta(delta) => {
                println!(
                    "  📚 Book: {} @ {} (qty: {})",
                    if delta.side == Side::Buy { "BUY" } else { "SELL" },
                    delta.price,
                    delta.level_qty_after
                );
            }
            EngineEvent::Lifecycle(lifecycle) => {
                println!(
                    "  🔄 Lifecycle: Order {} - {}",
                    lifecycle.order_id,
                    match lifecycle.kind {
                        LifecycleKind::Accepted => "ACCEPTED".green(),
                        LifecycleKind::Rejected =>
                            format!("REJECTED: {:?}", lifecycle.reason).red(),
                        LifecycleKind::Cancelled => "CANCELLED".yellow(),
                    }
                );
            }
            EngineEvent::TickComplete(_) => {
                println!("  ✅ Tick {} complete", tick);
            }
        }
    }
}

pub fn run_validation_tests(symbol: u32) {
    println!("{}", "Running Validation Tests...".green().bold());

    let cfg = EngineCfg {
        symbol,
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

    let mut engine = Whistle::new(cfg);
    let tick = 100;

    // Test various order types and scenarios
    let test_orders = vec![
        (1, Side::Buy, OrderType::Limit, Some(150), 10, "Basic limit buy"),
        (2, Side::Sell, OrderType::Limit, Some(155), 10, "Basic limit sell"),
        (3, Side::Buy, OrderType::Market, None, 5, "Market buy"),
        (4, Side::Sell, OrderType::PostOnly, Some(145), 8, "Post-only sell"),
        (5, Side::Buy, OrderType::Ioc, Some(160), 12, "IOC buy"),
    ];

    for (order_id, side, order_type, price, qty, description) in test_orders {
        println!("  Testing: {}", description);
        let msg = InboundMsg::submit(
            order_id as u64,
            1,
            side,
            order_type,
            price,
            qty,
            tick * 1000,
            0, // ts_norm
            0, // enq_seq
        );
        if let Err(e) = engine.enqueue_message(msg) {
            println!("Failed to enqueue order: {:?}", e);
        }
    }

    let events = engine.tick(tick);

    println!("{}", "Validation Results:".cyan().bold());
    for event in events {
        match event {
            EngineEvent::Trade(trade) => {
                println!(
                    "  💰 Trade: {} @ {} (exec_id: {})",
                    trade.qty, trade.price, trade.exec_id
                );
            }
            EngineEvent::BookDelta(delta) => {
                println!(
                    "  📚 Book: {} @ {} (qty: {})",
                    if delta.side == Side::Buy { "BUY" } else { "SELL" },
                    delta.price,
                    delta.level_qty_after
                );
            }
            EngineEvent::Lifecycle(lifecycle) => {
                println!(
                    "  🔄 Lifecycle: Order {} - {}",
                    lifecycle.order_id,
                    match lifecycle.kind {
                        LifecycleKind::Accepted => "ACCEPTED".green(),
                        LifecycleKind::Rejected =>
                            format!("REJECTED: {:?}", lifecycle.reason).red(),
                        LifecycleKind::Cancelled => "CANCELLED".yellow(),
                    }
                );
            }
            EngineEvent::TickComplete(_) => {
                println!("  ✅ Tick {} complete", tick);
            }
        }
    }
}

pub fn run_determinism_tests(symbol: u32) {
    println!("{}", "Running Determinism Tests...".green().bold());

    let cfg = EngineCfg {
        symbol,
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

    // Run the same sequence multiple times
    for run in 1..=3 {
        println!("  Run {}:", run);
        let mut engine = Whistle::new(cfg);
        let tick = 100;

        // Submit the same orders
        let orders = vec![
            (1, Side::Buy, OrderType::Limit, Some(150), 10),
            (2, Side::Sell, OrderType::Limit, Some(155), 10),
            (3, Side::Buy, OrderType::Market, None, 5),
        ];

        for (order_id, side, order_type, price, qty) in orders {
            let msg = InboundMsg::submit(
                order_id as u64,
                1,
                side,
                order_type,
                price,
                qty,
                tick * 1000,
                0, // ts_norm
                0, // enq_seq
            );
            if let Err(e) = engine.enqueue_message(msg) {
                println!("Failed to enqueue order: {:?}", e);
            }
        }

        let events = engine.tick(tick);

        for event in events {
            match event {
                EngineEvent::Trade(trade) => {
                    println!(
                        "    💰 Trade: {} @ {} (exec_id: {})",
                        trade.qty, trade.price, trade.exec_id
                    );
                }
                EngineEvent::BookDelta(delta) => {
                    println!(
                        "    📚 Book: {} @ {} (qty: {})",
                        if delta.side == Side::Buy { "BUY" } else { "SELL" },
                        delta.price,
                        delta.level_qty_after
                    );
                }
                EngineEvent::Lifecycle(lifecycle) => {
                    println!(
                        "    🔄 Lifecycle: Order {} - {}",
                        lifecycle.order_id,
                        match lifecycle.kind {
                            LifecycleKind::Accepted => "ACCEPTED".green(),
                            LifecycleKind::Rejected =>
                                format!("REJECTED: {:?}", lifecycle.reason).red(),
                            LifecycleKind::Cancelled => "CANCELLED".yellow(),
                        }
                    );
                }
                EngineEvent::TickComplete(_) => {
                    println!("    ✅ Tick {} complete", tick);
                }
            }
        }
    }
}

pub fn run_capacity_tests(symbol: u32) {
    println!("{}", "Running Capacity Tests...".green().bold());

    let cfg = EngineCfg {
        symbol,
        price_domain: PriceDomain { floor: 100, ceil: 200, tick: 5 },
        bands: Bands { mode: BandMode::Percent(1000) },
        batch_max: 8, // Small batch size for testing
        arena_capacity: 4096,
        elastic_arena: false,
        exec_shift_bits: 12,
        exec_id_mode: ExecIdMode::Sharded,
        self_match_policy: SelfMatchPolicy::Skip,
        allow_market_cold_start: false,
        reference_price_source: ReferencePriceSource::SnapshotLastTrade,
    };

    let mut engine = Whistle::new(cfg);
    let tick = 100;

    // Submit many orders to test capacity
    for i in 0..100 {
        let order_id = tick * 1000 + i;
        let msg = InboundMsg::submit(
            order_id as u64,
            (i % 5) as u64 + 1, // 5 different accounts
            if i % 2 == 0 { Side::Buy } else { Side::Sell },
            OrderType::Limit,
            Some((150 + (i % 10) * 5) as u32),
            10,
            tick * 1000 + i,
            0, // ts_norm
            0, // enq_seq
        );
        if let Err(e) = engine.enqueue_message(msg) {
            println!("Failed to enqueue order {}: {:?}", i, e);
        }
    }

    let events = engine.tick(tick);

    println!("{}", "Capacity Test Results:".cyan().bold());
    let mut trades = 0;
    let mut book_deltas = 0;
    let mut lifecycles = 0;

    for event in events {
        match event {
            EngineEvent::Trade(_) => trades += 1,
            EngineEvent::BookDelta(_) => book_deltas += 1,
            EngineEvent::Lifecycle(_) => lifecycles += 1,
            EngineEvent::TickComplete(_) => {
                println!("  ✅ Tick {} complete", tick);
            }
        }
    }

    println!(
        "  📊 Events: {} trades, {} book deltas, {} lifecycles",
        trades, book_deltas, lifecycles
    );
}

pub fn run_matching_tests(symbol: u32) {
    println!("{}", "Running Matching Tests...".green().bold());

    let cfg = EngineCfg {
        symbol,
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

    let mut engine = Whistle::new(cfg);
    let tick = 100;

    // Test price-time priority
    let orders = vec![
        (1, Side::Buy, OrderType::Limit, Some(150), 10, "First buy order"),
        (2, Side::Buy, OrderType::Limit, Some(150), 5, "Second buy order (same price)"),
        (3, Side::Sell, OrderType::Limit, Some(150), 8, "Sell order that should match"),
    ];

    for (order_id, side, order_type, price, qty, description) in orders {
        println!("  Submitting: {} (Order {})", description, order_id);
        let msg = InboundMsg::submit(
            order_id as u64,
            1,
            side,
            order_type,
            price,
            qty,
            tick * 1000,
            0, // ts_norm
            0, // enq_seq
        );
        if let Err(e) = engine.enqueue_message(msg) {
            println!("Failed to enqueue order: {:?}", e);
        }
    }

    let events = engine.tick(tick);

    println!("{}", "Matching Test Results:".cyan().bold());
    for event in events {
        match event {
            EngineEvent::Trade(trade) => {
                println!(
                    "  💰 Trade: {} @ {} (exec_id: {})",
                    trade.qty, trade.price, trade.exec_id
                );
            }
            EngineEvent::BookDelta(delta) => {
                println!(
                    "  📚 Book: {} @ {} (qty: {})",
                    if delta.side == Side::Buy { "BUY" } else { "SELL" },
                    delta.price,
                    delta.level_qty_after
                );
            }
            EngineEvent::Lifecycle(lifecycle) => {
                println!(
                    "  🔄 Lifecycle: Order {} - {}",
                    lifecycle.order_id,
                    match lifecycle.kind {
                        LifecycleKind::Accepted => "ACCEPTED".green(),
                        LifecycleKind::Rejected =>
                            format!("REJECTED: {:?}", lifecycle.reason).red(),
                        LifecycleKind::Cancelled => "CANCELLED".yellow(),
                    }
                );
            }
            EngineEvent::TickComplete(_) => {
                println!("  ✅ Tick {} complete", tick);
            }
        }
    }
}

pub fn run_post_only_tests(symbol: u32) {
    println!("{}", "Running POST-ONLY Tests...".green().bold());

    let cfg = EngineCfg {
        symbol,
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

    let mut engine = Whistle::new(cfg);
    let tick = 100;

    // Test POST-ONLY cross prevention
    let orders = vec![
        (1, Side::Buy, OrderType::Limit, Some(150), 10, "Existing buy order"),
        (
            2,
            Side::Sell,
            OrderType::PostOnly,
            Some(150),
            5,
            "POST-ONLY sell that should be rejected",
        ),
        (
            3,
            Side::Sell,
            OrderType::PostOnly,
            Some(155),
            5,
            "POST-ONLY sell that should be accepted",
        ),
    ];

    for (order_id, side, order_type, price, qty, description) in orders {
        println!("  Submitting: {} (Order {})", description, order_id);
        let msg = InboundMsg::submit(
            order_id as u64,
            1,
            side,
            order_type,
            price,
            qty,
            tick * 1000,
            0, // ts_norm
            0, // enq_seq
        );
        if let Err(e) = engine.enqueue_message(msg) {
            println!("Failed to enqueue order: {:?}", e);
        }
    }

    let events = engine.tick(tick);

    println!("{}", "POST-ONLY Test Results:".cyan().bold());
    for event in events {
        match event {
            EngineEvent::Trade(trade) => {
                println!(
                    "  💰 Trade: {} @ {} (exec_id: {})",
                    trade.qty, trade.price, trade.exec_id
                );
            }
            EngineEvent::BookDelta(delta) => {
                println!(
                    "  📚 Book: {} @ {} (qty: {})",
                    if delta.side == Side::Buy { "BUY" } else { "SELL" },
                    delta.price,
                    delta.level_qty_after
                );
            }
            EngineEvent::Lifecycle(lifecycle) => {
                println!(
                    "  🔄 Lifecycle: Order {} - {}",
                    lifecycle.order_id,
                    match lifecycle.kind {
                        LifecycleKind::Accepted => "ACCEPTED".green(),
                        LifecycleKind::Rejected =>
                            format!("REJECTED: {:?}", lifecycle.reason).red(),
                        LifecycleKind::Cancelled => "CANCELLED".yellow(),
                    }
                );
            }
            EngineEvent::TickComplete(_) => {
                println!("  ✅ Tick {} complete", tick);
            }
        }
    }
}

pub fn run_self_match_tests(symbol: u32) {
    println!("{}", "Running Self-Match Prevention Tests...".green().bold());

    let cfg = EngineCfg {
        symbol,
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

    let mut engine = Whistle::new(cfg);
    let tick = 100;

    // Test self-match prevention
    let orders = vec![
        (1, 1, Side::Buy, OrderType::Limit, Some(150), 10, "Buy order from account 1"),
        (
            2,
            1,
            Side::Sell,
            OrderType::Limit,
            Some(150),
            5,
            "Sell order from same account (should not match)",
        ),
        (
            3,
            2,
            Side::Sell,
            OrderType::Limit,
            Some(150),
            5,
            "Sell order from different account (should match)",
        ),
    ];

    for (order_id, account, side, order_type, price, qty, description) in orders {
        println!("  Submitting: {} (Order {} from Account {})", description, order_id, account);
        let msg = InboundMsg::submit(
            order_id as u64,
            account as u64,
            side,
            order_type,
            price,
            qty,
            tick * 1000,
            0, // ts_norm
            0, // enq_seq
        );
        if let Err(e) = engine.enqueue_message(msg) {
            println!("Failed to enqueue order: {:?}", e);
        }
    }

    let events = engine.tick(tick);

    println!("{}", "Self-Match Prevention Test Results:".cyan().bold());
    for event in events {
        match event {
            EngineEvent::Trade(trade) => {
                println!(
                    "  💰 Trade: {} @ {} (exec_id: {})",
                    trade.qty, trade.price, trade.exec_id
                );
            }
            EngineEvent::BookDelta(delta) => {
                println!(
                    "  📚 Book: {} @ {} (qty: {})",
                    if delta.side == Side::Buy { "BUY" } else { "SELL" },
                    delta.price,
                    delta.level_qty_after
                );
            }
            EngineEvent::Lifecycle(lifecycle) => {
                println!(
                    "  🔄 Lifecycle: Order {} - {}",
                    lifecycle.order_id,
                    match lifecycle.kind {
                        LifecycleKind::Accepted => "ACCEPTED".green(),
                        LifecycleKind::Rejected =>
                            format!("REJECTED: {:?}", lifecycle.reason).red(),
                        LifecycleKind::Cancelled => "CANCELLED".yellow(),
                    }
                );
            }
            EngineEvent::TickComplete(_) => {
                println!("  ✅ Tick {} complete", tick);
            }
        }
    }
}

pub fn run_order_type_tests(symbol: u32) {
    println!("{}", "Running Order Type Tests...".green().bold());

    let cfg = EngineCfg {
        symbol,
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

    let mut engine = Whistle::new(cfg);
    let tick = 100;

    // Test different order types
    let orders = vec![
        (1, Side::Buy, OrderType::Limit, Some(150), 10, "Limit order"),
        (2, Side::Sell, OrderType::Market, None, 5, "Market order"),
        (3, Side::Buy, OrderType::Ioc, Some(160), 8, "IOC order"),
        (4, Side::Sell, OrderType::PostOnly, Some(155), 12, "POST-ONLY order"),
    ];

    for (order_id, side, order_type, price, qty, description) in orders {
        println!("  Submitting: {} (Order {})", description, order_id);
        let msg = InboundMsg::submit(
            order_id as u64,
            1,
            side,
            order_type,
            price,
            qty,
            tick * 1000,
            0, // ts_norm
            0, // enq_seq
        );
        if let Err(e) = engine.enqueue_message(msg) {
            println!("Failed to enqueue order: {:?}", e);
        }
    }

    let events = engine.tick(tick);

    println!("{}", "Order Type Test Results:".cyan().bold());
    for event in events {
        match event {
            EngineEvent::Trade(trade) => {
                println!(
                    "  💰 Trade: {} @ {} (exec_id: {})",
                    trade.qty, trade.price, trade.exec_id
                );
            }
            EngineEvent::BookDelta(delta) => {
                println!(
                    "  📚 Book: {} @ {} (qty: {})",
                    if delta.side == Side::Buy { "BUY" } else { "SELL" },
                    delta.price,
                    delta.level_qty_after
                );
            }
            EngineEvent::Lifecycle(lifecycle) => {
                println!(
                    "  🔄 Lifecycle: Order {} - {}",
                    lifecycle.order_id,
                    match lifecycle.kind {
                        LifecycleKind::Accepted => "ACCEPTED".green(),
                        LifecycleKind::Rejected =>
                            format!("REJECTED: {:?}", lifecycle.reason).red(),
                        LifecycleKind::Cancelled => "CANCELLED".yellow(),
                    }
                );
            }
            EngineEvent::TickComplete(_) => {
                println!("  ✅ Tick {} complete", tick);
            }
        }
    }
}

pub fn run_event_ordering_tests(symbol: u32) {
    println!("{}", "Running Event Ordering Tests...".green().bold());

    let cfg = EngineCfg {
        symbol,
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

    let mut engine = Whistle::new(cfg);
    let tick = 100;

    // Submit orders that will generate multiple event types
    let orders = vec![
        (1, Side::Buy, OrderType::Limit, Some(150), 10),
        (2, Side::Sell, OrderType::Limit, Some(150), 5),
        (3, Side::Buy, OrderType::Market, None, 3),
    ];

    for (order_id, side, order_type, price, qty) in orders {
        let msg = InboundMsg::submit(
            order_id as u64,
            1,
            side,
            order_type,
            price,
            qty,
            tick * 1000,
            0, // ts_norm
            0, // enq_seq
        );
        if let Err(e) = engine.enqueue_message(msg) {
            println!("Failed to enqueue order: {:?}", e);
        }
    }

    let events = engine.tick(tick);

    println!("{}", "Event Ordering Test Results:".cyan().bold());
    let mut event_count = 0;
    for event in events {
        event_count += 1;
        match event {
            EngineEvent::Trade(trade) => {
                println!(
                    "  {}. 💰 Trade: {} @ {} (exec_id: {})",
                    event_count, trade.qty, trade.price, trade.exec_id
                );
            }
            EngineEvent::BookDelta(delta) => {
                println!(
                    "  {}. 📚 Book: {} @ {} (qty: {})",
                    event_count,
                    if delta.side == Side::Buy { "BUY" } else { "SELL" },
                    delta.price,
                    delta.level_qty_after
                );
            }
            EngineEvent::Lifecycle(lifecycle) => {
                println!(
                    "  {}. 🔄 Lifecycle: Order {} - {}",
                    event_count,
                    lifecycle.order_id,
                    match lifecycle.kind {
                        LifecycleKind::Accepted => "ACCEPTED".green(),
                        LifecycleKind::Rejected =>
                            format!("REJECTED: {:?}", lifecycle.reason).red(),
                        LifecycleKind::Cancelled => "CANCELLED".yellow(),
                    }
                );
            }
            EngineEvent::TickComplete(_) => {
                println!("  {}. ✅ Tick {} complete", event_count, tick);
            }
        }
    }
}
