use clap::{Parser, Subcommand};
use colored::*;
use whistle::{
    BandMode, Bands, EngineCfg, ExecIdMode, InboundMsg, OrderType, PriceDomain,
    ReferencePriceSource, SelfMatchPolicy, Side, Whistle,
};

#[derive(Parser)]
#[command(name = "whistle-playground")]
#[command(about = "Interactive playground for testing the Whistle matching engine")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start an interactive session with a Whistle engine
    Interactive {
        /// Symbol ID for the engine
        #[arg(short, long, default_value = "1")]
        symbol: u32,

        /// Price floor
        #[arg(long, default_value = "100")]
        price_floor: u32,

        /// Price ceiling
        #[arg(long, default_value = "200")]
        price_ceil: u32,

        /// Tick size
        #[arg(long, default_value = "5")]
        tick_size: u32,

        /// Batch max size
        #[arg(long, default_value = "1024")]
        batch_max: u32,

        /// Arena capacity
        #[arg(long, default_value = "4096")]
        arena_capacity: u32,
    },

    /// Run a quick demo of the engine
    Demo {
        /// Symbol ID for the demo
        #[arg(short, long, default_value = "42")]
        symbol: u32,
    },

    /// Run comprehensive validation tests
    TestValidation {
        /// Symbol ID for testing
        #[arg(short, long, default_value = "42")]
        symbol: u32,
    },

    /// Run determinism and replay tests
    TestDeterminism {
        /// Symbol ID for testing
        #[arg(short, long, default_value = "42")]
        symbol: u32,
    },

    /// Run capacity and backpressure tests
    TestCapacity {
        /// Symbol ID for testing
        #[arg(short, long, default_value = "42")]
        symbol: u32,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Interactive {
            symbol,
            price_floor,
            price_ceil,
            tick_size,
            batch_max,
            arena_capacity,
        } => {
            run_interactive(symbol, price_floor, price_ceil, tick_size, batch_max, arena_capacity);
        }
        Commands::Demo { symbol } => {
            run_demo(symbol);
        }
        Commands::TestValidation { symbol } => {
            run_validation_tests(symbol);
        }
        Commands::TestDeterminism { symbol } => {
            run_determinism_tests(symbol);
        }
        Commands::TestCapacity { symbol } => {
            run_capacity_tests(symbol);
        }
    }
}

fn run_interactive(
    symbol: u32,
    price_floor: u32,
    price_ceil: u32,
    tick_size: u32,
    batch_max: u32,
    arena_capacity: u32,
) {
    println!("{}", "🚀 Whistle Playground - Interactive Mode".green().bold());
    println!("Symbol: {symbol}, Price Range: {price_floor}-{price_ceil}, Tick: {tick_size}, Batch: {batch_max}, Arena: {arena_capacity}");
    println!("{}", "Type 'help' for available commands".yellow());
    println!();

    let cfg = EngineCfg {
        symbol,
        price_domain: PriceDomain { floor: price_floor, ceil: price_ceil, tick: tick_size },
        bands: Bands { mode: BandMode::Percent(1000) },
        batch_max,
        arena_capacity,
        elastic_arena: false,
        exec_shift_bits: 12,
        exec_id_mode: ExecIdMode::Sharded,
        self_match_policy: SelfMatchPolicy::Skip,
        allow_market_cold_start: false,
        reference_price_source: ReferencePriceSource::SnapshotLastTrade,
    };

    let mut engine = Whistle::new(cfg);
    let mut tick = 100u64;

    loop {
        print!("{} ", "whistle>".blue().bold());
        std::io::Write::flush(&mut std::io::stdout()).unwrap();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        match input {
            "help" | "h" => {
                print_help();
            }
            "quit" | "q" | "exit" => {
                println!("{}", "Goodbye! 👋".green());
                break;
            }
            "status" | "s" => {
                print_status(&engine, tick);
            }
            "tick" | "t" => {
                tick = process_tick(&mut engine, tick);
            }
            "submit" | "sub" => {
                submit_order_interactive(&mut engine);
            }
            "cancel" | "can" => {
                cancel_order_interactive(&mut engine);
            }
            "clear" | "c" => {
                engine.clear_queue();
                println!("{}", "Queue cleared".yellow());
            }
            "demo" | "d" => {
                run_quick_demo(&mut engine, &mut tick);
            }
            "validate" | "v" => {
                run_validation_demo(&mut engine, &mut tick);
            }
            "capacity" | "cap" => {
                run_capacity_demo(&mut engine, &mut tick);
            }
            _ => {
                if let Some(args) = input.strip_prefix("submit ") {
                    submit_order_from_args(&mut engine, args);
                } else if let Some(args) = input.strip_prefix("cancel ") {
                    cancel_order_from_args(&mut engine, args);
                } else if !input.is_empty() {
                    println!("{}", "Unknown command. Type 'help' for available commands.".red());
                }
            }
        }
    }
}

fn run_demo(symbol: u32) {
    println!("{}", "🎬 Whistle Playground - Demo Mode".green().bold());
    println!("Running quick demo with symbol {symbol}");
    println!();

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
    let mut tick = 100u64;

    run_quick_demo(&mut engine, &mut tick);
}

fn run_quick_demo(engine: &mut Whistle, tick: &mut u64) {
    println!("{}", "📝 Running demo sequence...".cyan());

    // Submit some orders
    let orders = vec![
        (
            "Buy Limit 150@10",
            InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1),
        ),
        (
            "Sell Limit 160@5",
            InboundMsg::submit(2, 2, Side::Sell, OrderType::Limit, Some(160), 5, 1001, 0, 2),
        ),
        (
            "Buy Market 20",
            InboundMsg::submit(3, 3, Side::Buy, OrderType::Market, None, 20, 1002, 0, 3),
        ),
        ("Cancel Order 2", InboundMsg::cancel(2, 1003, 4)),
    ];

    for (desc, msg) in orders {
        println!("  {desc}");
        match engine.enqueue_message(msg) {
            Ok(()) => println!("    {} ✓ Enqueued", "OK".green()),
            Err(e) => println!("    {} ✗ Failed: {:?}", "ERROR".red(), e),
        }
    }

    println!();
    println!("{}", "🔄 Processing tick...".cyan());
    let events = engine.tick(*tick);
    *tick += 1;

    println!("  Generated {} events:", events.len());
    for (i, event) in events.iter().enumerate() {
        println!("    {}. {:?}", i + 1, event);
    }

    println!();
    print_status(engine, *tick);
}

fn print_help() {
    println!("{}", "Available Commands:".cyan().bold());
    println!("  help, h          - Show this help");
    println!("  quit, q, exit    - Exit the playground");
    println!("  status, s        - Show engine status");
    println!("  tick, t          - Process next tick");
    println!("  submit, sub      - Submit order (interactive)");
    println!("  cancel, can      - Cancel order (interactive)");
    println!("  clear, c         - Clear the message queue");
    println!("  demo, d          - Run quick demo");
    println!("  validate, v      - Run validation demo");
    println!("  capacity, cap    - Run capacity demo");
    println!();
    println!("{}", "Quick Commands:".cyan().bold());
    println!("  submit <args>    - Submit order with arguments");
    println!("  cancel <order_id> - Cancel order by ID");
    println!();
    println!("{}", "Examples:".yellow());
    println!("  submit buy limit 150 10  # Buy 10 @ 150");
    println!("  submit sell market 5    # Sell 5 @ market");
    println!("  cancel 123              # Cancel order 123");
}

fn print_status(engine: &Whistle, tick: u64) {
    let (queue_len, queue_capacity) = engine.queue_stats();

    println!("{}", "📊 Engine Status".cyan().bold());
    println!("  Tick: {tick}");
    println!("  Queue: {queue_len}/{queue_capacity} messages");
    println!("  Symbol: {}", engine.symbol());
    println!(
        "  Price Domain: {}-{} (tick: {})",
        engine.price_domain().floor,
        engine.price_domain().ceil,
        engine.price_domain().tick
    );
}

fn process_tick(engine: &mut Whistle, tick: u64) -> u64 {
    println!("{}", "🔄 Processing tick...".cyan());
    let events = engine.tick(tick);

    if events.is_empty() {
        println!("  No events generated");
    } else {
        println!("  Generated {} events:", events.len());
        for (i, event) in events.iter().enumerate() {
            println!("    {}. {:?}", i + 1, event);
        }
    }

    tick + 1
}

fn submit_order_interactive(engine: &mut Whistle) {
    println!("{}", "📝 Submit Order".cyan().bold());

    // Get order details interactively
    let side = get_side_interactive();
    let order_type = get_order_type_interactive();
    let price = if order_type != OrderType::Market { Some(get_price_interactive()) } else { None };
    let qty = get_qty_interactive();

    let msg = InboundMsg::submit(
        generate_order_id(),
        generate_account_id(),
        side,
        order_type,
        price,
        qty,
        generate_timestamp(),
        0,
        generate_enq_seq(),
    );

    match engine.enqueue_message(msg) {
        Ok(()) => println!("{} ✓ Order submitted successfully", "OK".green()),
        Err(e) => println!("{} ✗ Failed to submit order: {:?}", "ERROR".red(), e),
    }
}

fn cancel_order_interactive(engine: &mut Whistle) {
    println!("{}", "❌ Cancel Order".cyan().bold());
    print!("Enter order ID to cancel: ");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    match input.trim().parse::<u64>() {
        Ok(order_id) => {
            let msg = InboundMsg::cancel(order_id, generate_timestamp(), generate_enq_seq());
            match engine.enqueue_message(msg) {
                Ok(()) => println!("{} ✓ Cancel request submitted", "OK".green()),
                Err(e) => println!("{} ✗ Failed to submit cancel: {:?}", "ERROR".red(), e),
            }
        }
        Err(_) => println!("{} ✗ Invalid order ID", "ERROR".red()),
    }
}

fn submit_order_from_args(engine: &mut Whistle, args: &str) {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 3 {
        println!("{} Usage: submit <side> <type> <qty> [price]", "ERROR".red());
        return;
    }

    let side = match parts[0].to_lowercase().as_str() {
        "buy" | "b" => Side::Buy,
        "sell" | "s" => Side::Sell,
        _ => {
            println!("{} Invalid side: {}", "ERROR".red(), parts[0]);
            return;
        }
    };

    let order_type = match parts[1].to_lowercase().as_str() {
        "limit" | "l" => OrderType::Limit,
        "market" | "m" => OrderType::Market,
        "ioc" => OrderType::Ioc,
        "postonly" | "po" => OrderType::PostOnly,
        _ => {
            println!("{} Invalid order type: {}", "ERROR".red(), parts[1]);
            return;
        }
    };

    let qty = match parts[2].parse::<u32>() {
        Ok(q) => q,
        Err(_) => {
            println!("{} Invalid quantity: {}", "ERROR".red(), parts[2]);
            return;
        }
    };

    let price = if order_type != OrderType::Market && parts.len() > 3 {
        match parts[3].parse::<u32>() {
            Ok(p) => Some(p),
            Err(_) => {
                println!("{} Invalid price: {}", "ERROR".red(), parts[3]);
                return;
            }
        }
    } else {
        None
    };

    let msg = InboundMsg::submit(
        generate_order_id(),
        generate_account_id(),
        side,
        order_type,
        price,
        qty,
        generate_timestamp(),
        0,
        generate_enq_seq(),
    );

    match engine.enqueue_message(msg) {
        Ok(()) => println!("{} ✓ Order submitted", "OK".green()),
        Err(e) => println!("{} ✗ Failed: {:?}", "ERROR".red(), e),
    }
}

fn cancel_order_from_args(engine: &mut Whistle, args: &str) {
    match args.parse::<u64>() {
        Ok(order_id) => {
            let msg = InboundMsg::cancel(order_id, generate_timestamp(), generate_enq_seq());
            match engine.enqueue_message(msg) {
                Ok(()) => println!("{} ✓ Cancel submitted", "OK".green()),
                Err(e) => println!("{} ✗ Failed: {:?}", "ERROR".red(), e),
            }
        }
        Err(_) => println!("{} ✗ Invalid order ID: {}", "ERROR".red(), args),
    }
}

// Helper functions for interactive input
fn get_side_interactive() -> Side {
    loop {
        print!("Side (buy/sell): ");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        match input.trim().to_lowercase().as_str() {
            "buy" | "b" => return Side::Buy,
            "sell" | "s" => return Side::Sell,
            _ => println!("{} Please enter 'buy' or 'sell'", "ERROR".red()),
        }
    }
}

fn get_order_type_interactive() -> OrderType {
    loop {
        print!("Order type (limit/market/ioc/postonly): ");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        match input.trim().to_lowercase().as_str() {
            "limit" | "l" => return OrderType::Limit,
            "market" | "m" => return OrderType::Market,
            "ioc" => return OrderType::Ioc,
            "postonly" | "po" => return OrderType::PostOnly,
            _ => println!("{} Please enter 'limit', 'market', 'ioc', or 'postonly'", "ERROR".red()),
        }
    }
}

fn get_price_interactive() -> u32 {
    loop {
        print!("Price: ");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        match input.trim().parse::<u32>() {
            Ok(price) => return price,
            Err(_) => println!("{} Please enter a valid price", "ERROR".red()),
        }
    }
}

fn get_qty_interactive() -> u32 {
    loop {
        print!("Quantity: ");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        match input.trim().parse::<u32>() {
            Ok(qty) => return qty,
            Err(_) => println!("{} Please enter a valid quantity", "ERROR".red()),
        }
    }
}

// Utility functions for generating test data
fn generate_order_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
}

fn generate_account_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    (SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() % 1000) as u64 + 1
}

fn generate_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

fn generate_enq_seq() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    (SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() % 1000) as u32
}

fn run_validation_tests(symbol: u32) {
    println!("{}", "🧪 Running Validation Tests".green().bold());
    println!("Testing order validation, price domain, and tick size compliance");
    println!();

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
    let tick = 100u64;

    // Test 1: Invalid tick size
    println!("{}", "Test 1: Invalid tick size (103 with tick=5)".cyan());
    let invalid_tick_msg =
        InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(103), 10, 1000, 0, 1);
    engine.enqueue_message(invalid_tick_msg).unwrap();
    let events = engine.tick(tick);

    let lifecycle_events: Vec<_> = events
        .iter()
        .filter_map(|e| if let whistle::EngineEvent::Lifecycle(ev) = e { Some(ev) } else { None })
        .collect();

    if let Some(rejected_event) = lifecycle_events.first() {
        println!("  ✓ Rejected with reason: {:?}", rejected_event.reason);
    } else {
        println!("  ✗ Expected rejection event");
    }
    println!();

    // Test 2: Price domain violation
    println!("{}", "Test 2: Price domain violation (price 50 < floor 100)".cyan());
    let invalid_price_msg =
        InboundMsg::submit(2, 2, Side::Buy, OrderType::Limit, Some(50), 10, 1001, 0, 2);
    engine.enqueue_message(invalid_price_msg).unwrap();
    let events = engine.tick(tick);

    let lifecycle_events: Vec<_> = events
        .iter()
        .filter_map(|e| if let whistle::EngineEvent::Lifecycle(ev) = e { Some(ev) } else { None })
        .collect();

    if let Some(rejected_event) = lifecycle_events.first() {
        println!("  ✓ Rejected with reason: {:?}", rejected_event.reason);
    } else {
        println!("  ✗ Expected rejection event");
    }
    println!();

    // Test 3: Valid order
    println!("{}", "Test 3: Valid order (price 150, tick-aligned)".cyan());
    let valid_msg =
        InboundMsg::submit(3, 3, Side::Buy, OrderType::Limit, Some(150), 10, 1002, 0, 3);
    engine.enqueue_message(valid_msg).unwrap();
    let events = engine.tick(tick);

    let lifecycle_events: Vec<_> = events
        .iter()
        .filter_map(|e| if let whistle::EngineEvent::Lifecycle(ev) = e { Some(ev) } else { None })
        .collect();

    if let Some(accepted_event) = lifecycle_events.first() {
        println!("  ✓ Accepted: {:?}", accepted_event.kind);
    } else {
        println!("  ✗ Expected acceptance event");
    }
    println!();

    println!("{}", "✅ Validation tests completed!".green().bold());
}

fn run_determinism_tests(symbol: u32) {
    println!("{}", "🔄 Running Determinism Tests".green().bold());
    println!("Testing that identical inputs produce identical outputs");
    println!();

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

    // Run the same sequence twice
    let mut engine1 = Whistle::new(cfg);
    let mut engine2 = Whistle::new(cfg);

    // Submit identical orders
    let msg1 = InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1);
    let msg2 = InboundMsg::submit(2, 2, Side::Sell, OrderType::Limit, Some(160), 5, 1001, 0, 2);

    engine1.enqueue_message(msg1.clone()).unwrap();
    engine1.enqueue_message(msg2.clone()).unwrap();
    engine2.enqueue_message(msg1).unwrap();
    engine2.enqueue_message(msg2).unwrap();

    let events1 = engine1.tick(100);
    let events2 = engine2.tick(100);

    // Compare events
    println!("{}", "Comparing event outputs...".cyan());
    println!("  Engine 1: {} events", events1.len());
    println!("  Engine 2: {} events", events2.len());

    if events1.len() == events2.len() {
        println!("  ✓ Event counts match");

        let mut all_match = true;
        for (i, (e1, e2)) in events1.iter().zip(events2.iter()).enumerate() {
            if format!("{e1:?}") != format!("{e2:?}") {
                println!("  ✗ Event {} differs", i + 1);
                all_match = false;
            }
        }

        if all_match {
            println!("  ✓ All events match exactly");
        }
    } else {
        println!("  ✗ Event counts differ");
    }
    println!();

    println!("{}", "✅ Determinism tests completed!".green().bold());
}

fn run_capacity_tests(symbol: u32) {
    println!("{}", "📊 Running Capacity Tests".green().bold());
    println!("Testing arena capacity limits and queue backpressure");
    println!();

    // Test 1: Arena capacity limits
    println!("{}", "Test 1: Arena capacity limits".cyan());
    let cfg = EngineCfg {
        symbol,
        price_domain: PriceDomain { floor: 100, ceil: 200, tick: 5 },
        bands: Bands { mode: BandMode::Percent(1000) },
        batch_max: 1024,
        arena_capacity: 8, // Small arena for testing
        elastic_arena: false,
        exec_shift_bits: 12,
        exec_id_mode: ExecIdMode::Sharded,
        self_match_policy: SelfMatchPolicy::Skip,
        allow_market_cold_start: false,
        reference_price_source: ReferencePriceSource::SnapshotLastTrade,
    };

    let mut engine = Whistle::new(cfg);
    let tick = 100u64;

    // Submit orders up to capacity
    for i in 1u32..=10 {
        let msg = InboundMsg::submit(
            i as u64,
            i as u64,
            Side::Buy,
            OrderType::Limit,
            Some(150 + i),
            10,
            1000 + i as u64,
            0,
            i,
        );
        match engine.enqueue_message(msg) {
            Ok(()) => println!("  Order {i}: Enqueued"),
            Err(e) => println!("  Order {i}: Rejected - {e:?}"),
        }
    }

    let events = engine.tick(tick);

    let lifecycle_events: Vec<_> = events
        .iter()
        .filter_map(|e| if let whistle::EngineEvent::Lifecycle(ev) = e { Some(ev) } else { None })
        .collect();

    println!("  Generated {} lifecycle events", lifecycle_events.len());
    println!();

    // Test 2: Queue backpressure
    println!("{}", "Test 2: Queue backpressure".cyan());
    let cfg = EngineCfg {
        symbol,
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

    let mut engine = Whistle::new(cfg);

    // Submit more messages than the queue can hold
    for i in 1u32..=5 {
        let msg = InboundMsg::submit(
            i as u64,
            i as u64,
            Side::Buy,
            OrderType::Limit,
            Some(150),
            10,
            1000 + i as u64,
            0,
            i,
        );
        match engine.enqueue_message(msg) {
            Ok(()) => println!("  Message {i}: Accepted"),
            Err(e) => println!("  Message {i}: Rejected - {e:?}"),
        }
    }
    println!();

    println!("{}", "✅ Capacity tests completed!".green().bold());
}

fn run_validation_demo(engine: &mut Whistle, tick: &mut u64) {
    println!("{}", "🧪 Running Validation Demo".cyan());

    // Test invalid tick size
    println!("  Testing invalid tick size (103 with tick=5)...");
    let invalid_tick_msg =
        InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(103), 10, 1000, 0, 1);
    engine.enqueue_message(invalid_tick_msg).unwrap();
    let events = engine.tick(*tick);
    *tick += 1;

    let lifecycle_events: Vec<_> = events
        .iter()
        .filter_map(|e| if let whistle::EngineEvent::Lifecycle(ev) = e { Some(ev) } else { None })
        .collect();

    if let Some(rejected_event) = lifecycle_events.first() {
        println!("    ✓ Rejected: {:?}", rejected_event.reason);
    }

    // Test valid order
    println!("  Testing valid order (price 150)...");
    let valid_msg =
        InboundMsg::submit(2, 2, Side::Buy, OrderType::Limit, Some(150), 10, 1001, 0, 2);
    engine.enqueue_message(valid_msg).unwrap();
    let events = engine.tick(*tick);
    *tick += 1;

    let lifecycle_events: Vec<_> = events
        .iter()
        .filter_map(|e| if let whistle::EngineEvent::Lifecycle(ev) = e { Some(ev) } else { None })
        .collect();

    if let Some(accepted_event) = lifecycle_events.first() {
        println!("    ✓ Accepted: {:?}", accepted_event.kind);
    }

    println!("{}", "✅ Validation demo completed!".green());
}

fn run_capacity_demo(engine: &mut Whistle, tick: &mut u64) {
    println!("{}", "📊 Running Capacity Demo".cyan());

    // Test queue backpressure
    println!("  Testing queue backpressure...");
    for i in 1u32..=5 {
        let msg = InboundMsg::submit(
            i as u64,
            i as u64,
            Side::Buy,
            OrderType::Limit,
            Some(150),
            10,
            1000 + i as u64,
            0,
            i,
        );
        match engine.enqueue_message(msg) {
            Ok(()) => println!("    Message {i}: Accepted"),
            Err(e) => println!("    Message {i}: Rejected - {e:?}"),
        }
    }

    // Process tick to see events
    println!("  Processing tick...");
    let events = engine.tick(*tick);
    *tick += 1;

    println!("    Generated {} events", events.len());
    for (i, event) in events.iter().enumerate() {
        println!("      {}. {:?}", i + 1, event);
    }

    println!("{}", "✅ Capacity demo completed!".green());
}
