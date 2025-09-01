use clap::{Parser, Subcommand};
use colored::*;
use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use whistle::{
    BandMode, Bands, EngineCfg, EngineEvent, ExecIdMode, OrderType, PriceDomain,
    ReferencePriceSource, SelfMatchPolicy, Side, Whistle,
};

use crate::engine::demos::*;
use crate::engine::playground::run_interactive;
use crate::session::SessionManager;

#[derive(Parser)]
#[command(name = "whistle-playground")]
#[command(about = "Interactive testing environment for Whistle matching engine")]
#[command(version = "0.1.0")]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start interactive playground
    Interactive {
        /// Symbol ID to trade
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

        /// Batch size
        #[arg(long, default_value = "1024")]
        batch_size: u32,

        /// Arena capacity
        #[arg(long, default_value = "4096")]
        arena_capacity: u32,

        /// Session name (optional)
        #[arg(long)]
        session: Option<String>,

        /// Account ID for session
        #[arg(long, default_value = "1")]
        account_id: u32,
    },

    /// List available sessions
    ListSessions,

    /// Show session information
    SessionInfo {
        /// Session name
        #[arg(required = true)]
        session_name: String,
    },

    /// Clean up expired sessions
    CleanupSessions,

    /// Create a new session
    CreateSession {
        /// Session name
        #[arg(required = true)]
        name: String,

        /// Number of accounts
        #[arg(short, long, default_value = "5")]
        accounts: u32,
    },

    /// Join an existing session
    JoinSession {
        /// Session name
        #[arg(required = true)]
        name: String,

        /// Account ID
        #[arg(short, long, default_value = "1")]
        account_id: u32,

        /// Account type
        #[arg(long, default_value = "trader")]
        account_type: String,
    },

    /// Submit an order to a session
    Submit {
        /// Session name
        #[arg(required = true)]
        session: String,

        /// Account ID
        #[arg(short, long, default_value = "1")]
        account_id: u32,

        /// Order ID (optional - will be auto-generated if not provided)
        #[arg(long)]
        order_id: Option<u64>,

        /// Side (buy/sell)
        #[arg(short, long)]
        side: String,

        /// Order type (limit/market/ioc/post-only)
        #[arg(long)]
        order_type: String,

        /// Price (required for limit orders)
        #[arg(short, long)]
        price: Option<u32>,

        /// Quantity
        #[arg(short, long)]
        qty: u32,
    },

    /// Switch to a different account in a session
    SwitchAccount {
        /// Session name
        #[arg(required = true)]
        session: String,

        /// Account ID to switch to
        #[arg(required = true)]
        account_id: u32,
    },

    /// Show account status (active orders, positions, etc.)
    AccountStatus {
        /// Session name
        #[arg(required = true)]
        session: String,

        /// Account ID (optional - uses current account if not specified)
        #[arg(short, long)]
        account_id: Option<u32>,
    },

    /// Interactive account trading mode
    AccountTrading {
        /// Session name
        #[arg(required = true)]
        session: String,

        /// Initial account ID
        #[arg(short, long, default_value = "1")]
        account_id: u32,
    },

    /// Demo commands
    Demo,
    TestValidation,
    TestDeterminism,
    TestCapacity,
    TestMatching,
    TestPostOnly,
    TestSelfMatch,
    TestOrderTypes,
    TestEventOrdering,
}

pub fn handle_commands(cli: Cli) {
    match cli.command {
        Commands::Interactive {
            symbol,
            price_floor,
            price_ceil,
            tick_size,
            batch_size,
            arena_capacity,
            session,
            account_id,
        } => {
            if let Some(session_name) = session {
                // Session-aware interactive mode
                run_session_interactive(
                    session_name,
                    account_id,
                    symbol,
                    price_floor,
                    price_ceil,
                    tick_size,
                    batch_size,
                    arena_capacity,
                );
            } else {
                // Regular interactive mode
                run_interactive(
                    symbol,
                    price_floor,
                    price_ceil,
                    tick_size,
                    batch_size,
                    arena_capacity,
                    None,
                    None,
                    1,
                );
            }
        }

        Commands::ListSessions => {
            list_sessions();
        }

        Commands::SessionInfo { session_name } => {
            show_session_info(&session_name);
        }

        Commands::CleanupSessions => {
            cleanup_sessions();
        }

        Commands::CreateSession { name, accounts } => {
            create_session(&name, accounts);
        }

        Commands::JoinSession { name, account_id, account_type } => {
            join_session(&name, account_id, &account_type);
        }

        Commands::Submit { session, account_id, order_id, side, order_type, price, qty } => {
            submit_order(&session, account_id, order_id, &side, &order_type, price, qty);
        }

        Commands::SwitchAccount { session, account_id } => {
            switch_account(&session, account_id);
        }

        Commands::AccountStatus { session, account_id } => {
            show_account_status(&session, account_id);
        }

        Commands::AccountTrading { session, account_id } => {
            run_account_trading(&session, account_id);
        }

        // Demo commands
        Commands::Demo => run_demo(1),
        Commands::TestValidation => run_validation_tests(1),
        Commands::TestDeterminism => run_determinism_tests(1),
        Commands::TestCapacity => run_capacity_tests(1),
        Commands::TestMatching => run_matching_tests(1),
        Commands::TestPostOnly => run_post_only_tests(1),
        Commands::TestSelfMatch => run_self_match_tests(1),
        Commands::TestOrderTypes => run_order_type_tests(1),
        Commands::TestEventOrdering => run_event_ordering_tests(1),
    }
}

fn run_session_interactive(
    session_name: String,
    account_id: u32,
    symbol: u32,
    price_floor: u32,
    price_ceil: u32,
    tick_size: u32,
    batch_size: u32,
    arena_capacity: u32,
) {
    let session_manager = SessionManager::new();

    // Ensure session exists
    if !session_manager.session_exists(&session_name) {
        println!("{}", "Session does not exist. Creating new session...".yellow());
        session_manager.create_session(&session_name, 5).unwrap();
    }

    // Join session
    match session_manager.join_session_with_type(&session_name, account_id, "trader") {
        Ok(_) => println!(
            "{}",
            format!("Joined session '{}' as account {}", session_name, account_id).green()
        ),
        Err(e) => {
            println!("{}", format!("Failed to join session: {}", e).red());
            return;
        }
    }

    println!("{}", format!("🚀 Whistle Playground - Session Mode").cyan().bold());
    println!("Session: {}, Account: {}, Symbol: {}", session_name, account_id, symbol);
    println!(
        "Price Range: {}-{}, Tick: {}, Batch: {}, Arena: {}",
        price_floor, price_ceil, tick_size, batch_size, arena_capacity
    );
    println!("Type 'help' for available commands");
    println!("Type 'submit <side> <type> <price> <qty>' to submit orders");
    println!("Type 'quit' to exit");

    // Start interactive loop with session awareness
    run_session_interactive_loop(
        session_name,
        account_id,
        symbol,
        price_floor,
        price_ceil,
        tick_size,
        batch_size,
        arena_capacity,
    );
}

fn run_session_interactive_loop(
    session_name: String,
    account_id: u32,
    symbol: u32,
    price_floor: u32,
    price_ceil: u32,
    tick_size: u32,
    batch_size: u32,
    arena_capacity: u32,
) {
    let mut engine = Whistle::new(EngineCfg {
        symbol,
        price_domain: PriceDomain { floor: price_floor, ceil: price_ceil, tick: tick_size },
        bands: Bands { mode: BandMode::Percent(1000) },
        batch_max: batch_size,
        arena_capacity,
        elastic_arena: false,
        exec_shift_bits: 12,
        exec_id_mode: ExecIdMode::Sharded,
        self_match_policy: SelfMatchPolicy::Skip,
        allow_market_cold_start: false,
        reference_price_source: ReferencePriceSource::SnapshotLastTrade,
    });

    let mut tick = 100;
    let mut order_id_counter = 1;

    loop {
        print!("whistle[{}:{}]> ", session_name, account_id);
        std::io::stdout().flush().unwrap();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        match input {
            "quit" | "exit" => {
                println!("{}", "Goodbye!".green());
                break;
            }
            "help" => {
                print_help();
            }
            "status" => {
                println!(
                    "Session: {}, Account: {}, Symbol: {}, Tick: {}",
                    session_name, account_id, symbol, tick
                );
            }
            "tick" => {
                let events = engine.tick(tick);
                println!("{}", format!("Tick {} processed", tick).cyan());
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
                                    whistle::LifecycleKind::Accepted => "ACCEPTED".green(),
                                    whistle::LifecycleKind::Rejected =>
                                        format!("REJECTED: {:?}", lifecycle.reason).red(),
                                    whistle::LifecycleKind::Cancelled => "CANCELLED".yellow(),
                                }
                            );
                        }
                        EngineEvent::TickComplete(_) => {
                            println!("  ✅ Tick complete");
                        }
                    }
                }
                tick += 1;
            }
            input if input.starts_with("submit ") => {
                let parts: Vec<&str> = input.split_whitespace().collect();
                if parts.len() >= 5 {
                    let side_str = parts[1];
                    let order_type_str = parts[2];
                    let price_str = parts[3];
                    let qty_str = parts[4];

                    let side = match side_str.to_lowercase().as_str() {
                        "buy" => Side::Buy,
                        "sell" => Side::Sell,
                        _ => {
                            println!("{}", "Invalid side. Use 'buy' or 'sell'".red());
                            continue;
                        }
                    };

                    let order_type = match order_type_str.to_lowercase().as_str() {
                        "limit" => OrderType::Limit,
                        "market" => OrderType::Market,
                        "ioc" => OrderType::Ioc,
                        "post-only" => OrderType::PostOnly,
                        _ => {
                            println!(
                                "{}",
                                "Invalid order type. Use 'limit', 'market', 'ioc', or 'post-only'"
                                    .red()
                            );
                            continue;
                        }
                    };

                    let price = if order_type == OrderType::Market {
                        None
                    } else {
                        match price_str.parse::<u32>() {
                            Ok(p) => Some(p),
                            Err(_) => {
                                println!("{}", "Invalid price".red());
                                continue;
                            }
                        }
                    };

                    let qty = match qty_str.parse::<u32>() {
                        Ok(q) => q,
                        Err(_) => {
                            println!("{}", "Invalid quantity".red());
                            continue;
                        }
                    };

                    // Submit order to session communication
                    let session_manager = SessionManager::new();
                    if let Err(e) = session_manager.submit_order_to_session(
                        &session_name,
                        account_id,
                        order_id_counter,
                        side,
                        order_type,
                        price,
                        qty,
                    ) {
                        println!("{}", format!("Failed to submit order: {}", e).red());
                    } else {
                        println!(
                            "{}",
                            format!("Order {} submitted to session", order_id_counter).green()
                        );
                        order_id_counter += 1;
                    }
                } else {
                    println!("{}", "Usage: submit <side> <type> <price> <qty>".yellow());
                }
            }
            _ => {
                println!("{}", "Unknown command. Type 'help' for available commands.".yellow());
            }
        }
    }
}

fn list_sessions() {
    let session_manager = SessionManager::new();
    let sessions = session_manager.list_sessions();

    if sessions.is_empty() {
        println!("{}", "No sessions found.".yellow());
        return;
    }

    println!("{}", "Available Sessions:".cyan().bold());
    for session in sessions {
        let age = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - session.created;

        let age_str = if age < 60 {
            format!("{}s ago", age)
        } else if age < 3600 {
            format!("{}m ago", age / 60)
        } else {
            format!("{}h ago", age / 3600)
        };

        println!("  📁 {} ({} accounts, created {})", session.name, session.accounts, age_str);
    }
}

fn show_session_info(session_name: &str) {
    let session_manager = SessionManager::new();

    match session_manager.get_session_info(session_name) {
        Some(session) => {
            println!("{}", format!("Session: {}", session.name).cyan().bold());
            println!("  Accounts: {}", session.accounts);
            println!("  Created: {}", format_timestamp(session.created));
            println!("  Last Activity: {}", format_timestamp(session.last_activity));

            if !session.participants.is_empty() {
                println!("  Participants:");
                for (account_id, participant) in &session.participants {
                    let status = if participant.connected { "🟢" } else { "🔴" };
                    println!(
                        "    {} Account {} ({}) - {}",
                        status,
                        account_id,
                        participant.name,
                        participant.account_type.to_string()
                    );
                }
            }
        }
        None => {
            println!("{}", format!("Session '{}' not found.", session_name).red());
        }
    }
}

fn cleanup_sessions() {
    let mut session_manager = SessionManager::new();
    let removed = session_manager.cleanup_expired_sessions();

    if removed > 0 {
        println!("{}", format!("Cleaned up {} expired sessions.", removed).green());
    } else {
        println!("{}", "No expired sessions to clean up.".yellow());
    }
}

fn create_session(name: &str, accounts: u32) {
    let session_manager = SessionManager::new();

    match session_manager.create_session(name, accounts) {
        Ok(_) => {
            println!(
                "{}",
                format!("Session '{}' created with {} accounts.", name, accounts).green()
            );
        }
        Err(e) => {
            println!("{}", format!("Failed to create session: {}", e).red());
        }
    }
}

fn join_session(name: &str, account_id: u32, account_type: &str) {
    let session_manager = SessionManager::new();

    match session_manager.join_session_with_type(name, account_id, account_type) {
        Ok(_) => {
            println!(
                "{}",
                format!("Joined session '{}' as account {} ({})", name, account_id, account_type)
                    .green()
            );
        }
        Err(e) => {
            println!("{}", format!("Failed to join session: {}", e).red());
        }
    }
}

fn submit_order(
    session: &str,
    account_id: u32,
    order_id: Option<u64>,
    side: &str,
    order_type: &str,
    price: Option<u32>,
    qty: u32,
) {
    let side = match side.to_lowercase().as_str() {
        "buy" => Side::Buy,
        "sell" => Side::Sell,
        _ => {
            println!("{}", "Invalid side. Use 'buy' or 'sell'".red());
            return;
        }
    };

    let order_type = match order_type.to_lowercase().as_str() {
        "limit" => OrderType::Limit,
        "market" => OrderType::Market,
        "ioc" => OrderType::Ioc,
        "post-only" => OrderType::PostOnly,
        _ => {
            println!(
                "{}",
                "Invalid order type. Use 'limit', 'market', 'ioc', or 'post-only'".red()
            );
            return;
        }
    };

    // Generate order ID if not provided
    let final_order_id = order_id.unwrap_or_else(|| {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        timestamp * 1000 + account_id as u64
    });

    let session_manager = SessionManager::new();

    match session_manager
        .submit_order_to_session(session, account_id, final_order_id, side, order_type, price, qty)
    {
        Ok(_) => {
            println!(
                "{}",
                format!("Order {} submitted to session '{}'", final_order_id, session).green()
            );
        }
        Err(e) => {
            println!("{}", format!("Failed to submit order: {}", e).red());
        }
    }
}

fn print_help() {
    println!("{}", "Available Commands:".cyan().bold());
    println!("  help                    - Show this help");
    println!("  status                  - Show current status");
    println!("  tick                    - Process current tick");
    println!("  submit <side> <type> <price> <qty> - Submit an order");
    println!("    side: buy/sell");
    println!("    type: limit/market/ioc/post-only");
    println!("    price: price (not needed for market orders)");
    println!("    qty: quantity");
    println!("  quit/exit               - Exit the playground");
}

fn format_timestamp(timestamp: u64) -> String {
    let datetime = SystemTime::UNIX_EPOCH + Duration::from_secs(timestamp);
    let datetime: chrono::DateTime<chrono::Utc> = datetime.into();
    datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

// Account management functions
fn switch_account(session: &str, account_id: u32) {
    // For now, just print the switch - in a real implementation, this would store state
    println!("{}", format!("Switched to account {} in session '{}'", account_id, session).green());
    println!("💡 Use 'account-status' to see account details");
    println!("💡 Use 'account-trading' to enter interactive trading mode");
}

fn show_account_status(session: &str, account_id: Option<u32>) {
    let current_account = account_id.unwrap_or(1); // Default to account 1 for now
    
    println!("{}", format!("📊 Account Status - Session: {}", session).cyan().bold());
    println!("Account ID: {}", current_account);
    println!();
    
    // Read from session files to show actual account data
    let sessions_dir = std::env::temp_dir().join("whistle-exchange");
    let session_dir = sessions_dir.join(session);
    
    // Show recent trades for this account
    println!("{}", "Recent Trades:".yellow());
    let trades_file = session_dir.join("trades.jsonl");
    if trades_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&trades_file) {
            let mut account_trades = Vec::new();
            for line in content.lines() {
                if let Ok(trade_data) = serde_json::from_str::<serde_json::Value>(line) {
                    // For now, show all trades since we don't have account ownership tracking yet
                    if let (Some(price), Some(qty), Some(taker_side)) = (
                        trade_data["price"].as_u64(),
                        trade_data["qty"].as_u64(),
                        trade_data["taker_side"].as_str()
                    ) {
                        account_trades.push((price, qty, taker_side.to_string()));
                    }
                }
            }
            
            if account_trades.is_empty() {
                println!("  No recent trades found");
            } else {
                // Show most recent trades first (reverse order)
                for (price, qty, side) in account_trades.iter().rev().take(5) {
                    let side_emoji = if side == "buy" { "🟢" } else { "🔴" };
                    println!("  {} {} @ {} ({} units)", side_emoji, side.to_uppercase(), price, qty);
                }
            }
        }
    } else {
        println!("  No recent trades found");
    }
    println!();
    
    // Show current order book state (aggregated from all updates)
    println!("{}", "Current Order Book:".yellow());
    let book_file = session_dir.join("book_updates.jsonl");
    if book_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&book_file) {
            // Use a HashMap to aggregate quantities by price level
            use std::collections::HashMap;
            let mut sell_levels: HashMap<u64, u64> = HashMap::new();
            let mut buy_levels: HashMap<u64, u64> = HashMap::new();
            
            for line in content.lines() {
                if let Ok(book_data) = serde_json::from_str::<serde_json::Value>(line) {
                    if let (Some(side), Some(price), Some(qty)) = (
                        book_data["side"].as_str(),
                        book_data["price"].as_u64(),
                        book_data["qty"].as_u64()
                    ) {
                        match side {
                            "sell" => {
                                if qty > 0 {
                                    sell_levels.insert(price, qty);
                                } else {
                                    sell_levels.remove(&price);
                                }
                            },
                            "buy" => {
                                if qty > 0 {
                                    buy_levels.insert(price, qty);
                                } else {
                                    buy_levels.remove(&price);
                                }
                            },
                            _ => {}
                        }
                    }
                }
            }
            
            // Convert to vectors and sort
            let mut sells: Vec<(u64, u64)> = sell_levels.into_iter().collect();
            let mut buys: Vec<(u64, u64)> = buy_levels.into_iter().collect();
            
            sells.sort_by(|a, b| a.0.cmp(&b.0)); // ascending for sells
            buys.sort_by(|a, b| b.0.cmp(&a.0));  // descending for buys
            
            println!("  Sells (Asks):");
            for (price, qty) in sells.iter().take(5) {
                println!("    {} @ {} ({} units)", "🔴", price, qty);
            }
            if sells.is_empty() {
                println!("    No sell orders");
            }
            
            println!("  Buys (Bids):");
            for (price, qty) in buys.iter().take(5) {
                println!("    {} @ {} ({} units)", "🟢", price, qty);
            }
            if buys.is_empty() {
                println!("    No buy orders");
            }
        }
    } else {
        println!("  No order book data available");
    }
    println!();
    
    // Show session statistics
    println!("{}", "Session Info:".yellow());
    println!("  📁 Session Directory: {}", session_dir.display());
    println!("  📄 Trades File: {} bytes", 
        if trades_file.exists() { 
            std::fs::metadata(&trades_file).map(|m| m.len()).unwrap_or(0) 
        } else { 0 });
    println!("  📄 Book File: {} bytes", 
        if book_file.exists() { 
            std::fs::metadata(&book_file).map(|m| m.len()).unwrap_or(0) 
        } else { 0 });
    println!();
    
    println!("💡 Use 'submit' to place orders");
    println!("💡 Use 'switch-account' to change accounts");
    println!("💡 Use 'account-status' to refresh this view");
}

fn run_account_trading(session: &str, account_id: u32) {
    println!("{}", format!("🚀 Account Trading Mode - Session: {}", session).cyan().bold());
    println!("Account: {}", account_id);
    println!("Type 'help' for available commands");
    println!("Type 'quit' to exit");
    println!();
    
    // TODO: Implement interactive account trading loop
    println!("💡 Interactive account trading coming soon!");
    println!("💡 For now, use 'submit' command to place orders");
}
