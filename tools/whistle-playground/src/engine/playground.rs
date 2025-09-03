use crate::session::SessionManager;
use colored::*;
use std::io::{self, Write};
use std::time::SystemTime;
use whistle::{
    BandMode, Bands, EngineCfg, EngineEvent, ExecIdMode, InboundMsg, LifecycleKind, OrderType,
    PriceDomain, ReferencePriceSource, SelfMatchPolicy, Side, Whistle,
};

pub fn run_interactive(
    symbol: u32,
    price_floor: u32,
    price_ceil: u32,
    tick_size: u32,
    batch_max: u32,
    arena_capacity: u32,
    session_name: Option<String>,
    account_id: Option<u32>,
    num_accounts: u32,
) {
    println!("{}", "🚀 Whistle Playground - Interactive Mode".green().bold());
    println!("Symbol: {symbol}, Price Range: {price_floor}-{price_ceil}, Tick: {tick_size}, Batch: {batch_max}, Arena: {arena_capacity}");
    println!("{}", "Type 'help' for available commands".yellow());
    println!();

    let mut session_manager = SessionManager::new();
    if let Some(name) = session_name {
        if let Some(acc) = account_id {
            session_manager.join_session(&name, acc).unwrap_or_else(|e| {
                eprintln!("{}", e.red());
                std::process::exit(1);
            });
        } else {
            session_manager.create_session(&name, num_accounts, 1).unwrap_or_else(|e| {
                eprintln!("{}", e.red());
                std::process::exit(1);
            });
        }
    } else {
        // Fallback to single-account mode if no session
        println!("{}", "Running in single-account mode (no session specified).".yellow());
        println!("{}", "You can use 'session' command to manage sessions.".yellow());
        println!();
    }

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
    let mut tick = 100;

    loop {
        print!("{} ", "whistle> ".blue().bold());
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        match input {
            "quit" | "exit" | "q" => {
                println!("{}", "Goodbye!".green());
                break;
            }
            "help" | "h" => {
                print_help();
            }
            "tick" | "t" => {
                tick_engine(&mut engine, &mut tick);
            }
            "book" | "b" => {
                print_order_book(&engine);
            }
            "demo" | "d" => {
                run_demo(&mut engine, &mut tick);
            }
            "validate" | "v" => {
                run_validation_demo(&mut engine, &mut tick);
            }
            "capacity" | "cap" => {
                run_capacity_demo(&mut engine, &mut tick);
            }
            "session" => {
                manage_sessions(&mut session_manager);
            }
            _ => {
                if let Some(args) = input.strip_prefix("submit ") {
                    handle_submit_command(&mut engine, &mut tick, args);
                } else if !input.is_empty() {
                    println!("{}", "Unknown command. Type 'help' for available commands.".red());
                }
            }
        }
    }
}

fn print_help() {
    println!("{}", "Available Commands:".cyan().bold());
    println!("  help, h         - Show this help");
    println!("  quit, exit, q   - Exit the playground");
    println!("  tick, t         - Process one tick");
    println!("  book, b         - Show order book");
    println!("  submit <args>   - Submit an order");
    println!("  demo, d         - Run demo");
    println!("  validate, v     - Run validation demo");
    println!("  capacity, cap   - Run capacity demo");
    println!("  session         - Manage sessions");
    println!();
    println!("{}", "Quick Commands:".cyan().bold());
    println!("  submit buy limit 150 10     - Buy 10 @ 150");
    println!("  submit sell market 0 5      - Sell 5 @ market");
    println!("  submit buy post_only 155 8  - Buy 8 @ 155 (post-only)");
    println!("  submit sell ioc 145 12      - Sell 12 @ 145 (IOC)");
}

fn tick_engine(engine: &mut Whistle, tick: &mut u32) {
    println!("{}", "Processing tick...".yellow());

    let events = engine.tick(*tick as u64);

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
                println!("  ✅ Tick {} complete", *tick);
            }
        }
    }

    *tick += 1;
}

fn print_order_book(engine: &Whistle) {
    println!("{}", "📚 Order Book:".cyan().bold());

    // Get order book levels
    let bids = engine.get_order_book_levels(Side::Buy);
    let asks = engine.get_order_book_levels(Side::Sell);

    if !asks.is_empty() {
        println!("  Best Ask: {} @ {}", asks[0].1, asks[0].0);
    } else {
        println!("  Best Ask: None");
    }

    if !bids.is_empty() {
        println!("  Best Bid: {} @ {}", bids[0].1, bids[0].0);
    } else {
        println!("  Best Bid: None");
    }
}

fn handle_submit_command(engine: &mut Whistle, tick: &mut u32, args: &str) {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 4 {
        println!("{}", "Usage: submit <side> <type> <price> <qty>".red());
        println!("  side: buy/sell");
        println!("  type: limit/market/ioc/post_only");
        println!("  price: price (0 for market)");
        println!("  qty: quantity");
        return;
    }

    let side = match parts[0].to_lowercase().as_str() {
        "buy" => Side::Buy,
        "sell" => Side::Sell,
        _ => {
            println!("{}", "Invalid side. Use 'buy' or 'sell'.".red());
            return;
        }
    };

    let order_type = match parts[1].to_lowercase().as_str() {
        "limit" => OrderType::Limit,
        "market" => OrderType::Market,
        "ioc" => OrderType::Ioc,
        "post_only" => OrderType::PostOnly,
        _ => {
            println!(
                "{}",
                "Invalid order type. Use 'limit', 'market', 'ioc', or 'post_only'.".red()
            );
            return;
        }
    };

    let price: u32 = parts[2].parse().unwrap_or(0);
    let qty: u32 = parts[3].parse().unwrap_or(0);

    if qty == 0 {
        println!("{}", "Invalid quantity.".red());
        return;
    }

    let order_id = (*tick * 1000 + 1) as u64;
    let account_id = 1u64;

    let msg = InboundMsg::submit(
        order_id,
        account_id,
        side,
        order_type,
        if price > 0 { Some(price) } else { None },
        qty,
        (*tick * 1000) as u64,
        0, // ts_norm
        0, // enq_seq
    );

    match engine.enqueue_message(msg) {
        Ok(_) => println!(
            "{}",
            format!(
                "Submitted order: {} {} {} @ {}",
                qty,
                parts[0],
                parts[1],
                if price > 0 { price.to_string() } else { "market".to_string() }
            )
            .green()
        ),
        Err(e) => println!("{}", format!("Failed to submit order: {:?}", e).red()),
    }
}

fn run_demo(engine: &mut Whistle, tick: &mut u32) {
    println!("{}", "Running demo...".yellow());

    // Submit some orders
    let orders = vec![
        ("buy", "limit", 150, 10),
        ("sell", "limit", 155, 10),
        ("buy", "limit", 145, 5),
        ("sell", "limit", 160, 5),
    ];

    for (i, (side, order_type, price, qty)) in orders.iter().enumerate() {
        let order_id = (*tick * 1000 + i as u32 + 1) as u64;
        let msg = InboundMsg::submit(
            order_id,
            1,
            if *side == "buy" { Side::Buy } else { Side::Sell },
            match *order_type {
                "limit" => OrderType::Limit,
                "market" => OrderType::Market,
                "ioc" => OrderType::Ioc,
                "post_only" => OrderType::PostOnly,
                _ => OrderType::Limit,
            },
            Some(*price),
            *qty,
            (*tick * 1000) as u64,
            0, // ts_norm
            0, // enq_seq
        );
        if let Err(e) = engine.enqueue_message(msg) {
            println!("{}", format!("Failed to submit order: {:?}", e).red());
        }
    }

    tick_engine(engine, tick);
}

fn run_validation_demo(engine: &mut Whistle, tick: &mut u32) {
    println!("{}", "Running validation demo...".yellow());

    // Test various order types and scenarios
    let test_orders = vec![
        ("buy", "limit", 150, 10, "Basic limit buy"),
        ("sell", "limit", 155, 10, "Basic limit sell"),
        ("buy", "market", 0, 5, "Market buy"),
        ("sell", "post_only", 145, 8, "Post-only sell"),
        ("buy", "ioc", 160, 12, "IOC buy"),
    ];

    for (i, (side, order_type, price, qty, description)) in test_orders.iter().enumerate() {
        println!("  Testing: {}", description);
        let order_id = (*tick * 1000 + i as u32 + 1) as u64;
        let msg = InboundMsg::submit(
            order_id,
            1,
            if *side == "buy" { Side::Buy } else { Side::Sell },
            match *order_type {
                "limit" => OrderType::Limit,
                "market" => OrderType::Market,
                "ioc" => OrderType::Ioc,
                "post_only" => OrderType::PostOnly,
                _ => OrderType::Limit,
            },
            if *price > 0 { Some(*price) } else { None },
            *qty,
            (*tick * 1000) as u64,
            0, // ts_norm
            0, // enq_seq
        );
        if let Err(e) = engine.enqueue_message(msg) {
            println!("{}", format!("Failed to submit order: {:?}", e).red());
        }
    }

    tick_engine(engine, tick);
}

fn run_capacity_demo(engine: &mut Whistle, tick: &mut u32) {
    println!("{}", "Running capacity demo...".yellow());

    // Submit many orders to test capacity
    for i in 0..100 {
        let order_id = (*tick * 1000 + i + 1) as u64;
        let msg = InboundMsg::submit(
            order_id,
            ((i % 5) + 1) as u64, // 5 different accounts
            if i % 2 == 0 { Side::Buy } else { Side::Sell },
            OrderType::Limit,
            Some(150 + (i % 10) * 5),
            10,
            (*tick * 1000 + i) as u64,
            0, // ts_norm
            0, // enq_seq
        );
        if let Err(e) = engine.enqueue_message(msg) {
            println!("{}", format!("Failed to submit order {}: {:?}", i, e).red());
        }
    }

    tick_engine(engine, tick);
}

fn manage_sessions(session_manager: &mut SessionManager) {
    let mut exit = false;

    while !exit {
        println!("{}", "Session Management".cyan().bold());
        println!("1. Create New Session");
        println!("2. Join Existing Session");
        println!("3. List Sessions");
        println!("4. Show Session Info");
        println!("5. Clean Up Expired Sessions");
        println!("6. Exit");
        print!("{} ", "Enter choice: ".blue().bold());
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        match input {
            "1" => {
                print!("Session name: ");
                io::stdout().flush().unwrap();
                let mut name = String::new();
                io::stdin().read_line(&mut name).unwrap();
                name = name.trim().to_string();

                print!("Number of accounts (default 5): ");
                io::stdout().flush().unwrap();
                let mut accounts_str = String::new();
                io::stdin().read_line(&mut accounts_str).unwrap();
                let accounts: u32 = accounts_str.trim().parse().unwrap_or(5);

                match session_manager.create_session(&name, accounts, 1) {
                    Ok(_) => println!("{}", "Session '{}' created successfully!".green()),
                    Err(e) => println!("{}", e.red()),
                }
            }
            "2" => {
                print!("Session name: ");
                io::stdout().flush().unwrap();
                let mut name = String::new();
                io::stdin().read_line(&mut name).unwrap();
                name = name.trim().to_string();

                print!("Account ID (1-10): ");
                io::stdout().flush().unwrap();
                let mut account_str = String::new();
                io::stdin().read_line(&mut account_str).unwrap();
                let account: u32 = account_str.trim().parse().unwrap();

                match session_manager.join_session(&name, account) {
                    Ok(_) => println!(
                        "{}",
                        format!("Account {} joined session '{}'!", account, name).green()
                    ),
                    Err(e) => println!("{}", e.red()),
                }
            }
            "3" => {
                let sessions = session_manager.list_sessions();
                if sessions.is_empty() {
                    println!("{}", "No sessions found.".yellow());
                } else {
                    println!("{}", "Available Sessions:".cyan().bold());
                    for (i, session) in sessions.iter().enumerate() {
                        println!("  {}:", i + 1);
                        println!("    Name: {}", session.name);
                        println!("    Accounts: {}", session.accounts);
                        println!(
                            "    Created: {}",
                            SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                                - session.created
                        );
                        println!(
                            "    Last Activity: {}",
                            SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                                - session.last_activity
                        );
                        println!("    Participants:");
                        for (id, participant) in &session.participants {
                            println!("      Account {}: {}", id, participant.name);
                        }
                        println!();
                    }
                }
            }
            "4" => {
                print!("Session name: ");
                io::stdout().flush().unwrap();
                let mut name = String::new();
                io::stdin().read_line(&mut name).unwrap();
                name = name.trim().to_string();

                match session_manager.get_session_info(&name) {
                    Some(config) => {
                        println!("{}", "Session Info".cyan().bold());
                        println!("  Name: {}", config.name);
                        println!("  Accounts: {}", config.accounts);
                        println!(
                            "  Created: {}",
                            SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                                - config.created
                        );
                        println!(
                            "  Last Activity: {}",
                            SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                                - config.last_activity
                        );
                        println!("  Participants:");
                        for (id, participant) in &config.participants {
                            println!("    Account {}: {}", id, participant.name);
                        }
                    }
                    None => println!("{}", format!("Session '{}' not found.", name).red()),
                }
            }
            "5" => {
                let cleaned = session_manager.cleanup_expired_sessions();
                if cleaned > 0 {
                    println!("{}", format!("Cleaned up {} expired sessions.", cleaned).green());
                } else {
                    println!("{}", "No expired sessions to clean up.".yellow());
                }
            }
            "6" => {
                exit = true;
                println!("{}", "Exiting session management.".green());
            }
            _ => println!("{}", "Invalid choice. Please try again.".red()),
        }
    }
}
