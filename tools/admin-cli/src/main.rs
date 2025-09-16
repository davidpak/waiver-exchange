//! Waiver Exchange Admin CLI
//!
//! Unified admin interface with three modes:
//! - dashboard: Real-time order book display
//! - interactive: Order submission commands
//! - analytics: Analytics and metrics display

use clap::{Parser, Subcommand};
use colored::*;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
// use lazy_static::lazy_static;

use execution_manager::{ExecManagerConfig, ExecutionManager};
use order_router::{OrderRouter, RouterConfig};
use symbol_coordinator::{CoordinatorConfig, SymbolCoordinator, SymbolCoordinatorApi};
use whistle::{AccountId, EngineEvent, InboundMsg, OrderType, Price, Qty, Side, TickId};

#[derive(Parser)]
#[command(name = "admin-cli")]
#[command(about = "Unified admin CLI for Waiver Exchange - dashboard, orders, and analytics")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Real-time dashboard showing order books and system status
    Dashboard {
        /// Update frequency in milliseconds
        #[arg(short, long, default_value = "100")]
        update_ms: u64,
    },

    /// Interactive mode for submitting orders and system management
    Interactive,

    /// Analytics and metrics display
    Analytics {
        /// Update frequency in milliseconds
        #[arg(short, long, default_value = "1000")]
        update_ms: u64,
    },

    /// Live dashboard with order submission and real-time updates
    Live {
        /// Update frequency in milliseconds
        #[arg(short, long, default_value = "100")]
        update_ms: u64,
    },

    /// Main menu with all available options
    Menu,
}

#[derive(Debug, Clone)]
pub struct MarketData {
    #[allow(dead_code)]
    symbol: u32,
    last_trade_price: Option<u32>,
    last_trade_qty: Option<u64>,
    last_trade_time: Option<u64>,
    bid_price: Option<u32>,
    ask_price: Option<u32>,
    bid_qty: Option<u64>,
    ask_qty: Option<u64>,
    trades: Vec<Trade>,
    book_deltas: Vec<BookDelta>,
}

#[derive(Debug, Clone)]
struct Trade {
    #[allow(dead_code)]
    price: u32,
    #[allow(dead_code)]
    qty: u64,
    #[allow(dead_code)]
    side: Side,
    #[allow(dead_code)]
    timestamp: u64,
    #[allow(dead_code)]
    exec_id: u64,
}

#[derive(Debug, Clone)]
struct BookDelta {
    #[allow(dead_code)]
    side: Side,
    #[allow(dead_code)]
    price: u32,
    #[allow(dead_code)]
    qty: u64,
    #[allow(dead_code)]
    timestamp: u64,
}

/// Global system state shared across all modes
/// Uses proper production architecture: OrderRouter → SymbolCoordinator → Whistle → ExecutionManager
pub struct SystemState {
    pub execution_manager: Arc<ExecutionManager>,
    pub coordinator: Arc<Mutex<SymbolCoordinator>>,
    pub router: Arc<Mutex<OrderRouter>>,
    pub market_data: Arc<RwLock<HashMap<u32, MarketData>>>,
    pub current_tick: Arc<Mutex<TickId>>,
    pub active_symbols: Arc<RwLock<Vec<u32>>>,
}

impl Default for SystemState {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemState {
    pub fn new() -> Self {
        println!("🚀 Initializing Waiver Exchange System...");

        // Create ExecutionManager
        let exec_config = ExecManagerConfig::default();
        let execution_manager = Arc::new(ExecutionManager::new(exec_config));
        println!("✅ ExecutionManager created");

        // Create SymbolCoordinator
        let coord_config =
            CoordinatorConfig { num_threads: 4, spsc_depth: 1024, max_symbols_per_thread: 16 };
        let coordinator =
            Arc::new(Mutex::new(SymbolCoordinator::new(coord_config, execution_manager.clone())));
        println!("✅ SymbolCoordinator created");

        // Create OrderRouter
        let router_config = RouterConfig::default();
        let router = Arc::new(Mutex::new(OrderRouter::new(router_config)));
        println!("✅ OrderRouter created");

        // Initialize market data for common symbols (engines will be created on-demand via SymbolCoordinator)
        let mut market_data = HashMap::new();
        for symbol_id in 1..=10 {
            market_data.insert(
                symbol_id,
                MarketData {
                    symbol: symbol_id,
                    last_trade_price: None,
                    last_trade_qty: None,
                    last_trade_time: None,
                    bid_price: None,
                    ask_price: None,
                    bid_qty: None,
                    ask_qty: None,
                    trades: Vec::new(),
                    book_deltas: Vec::new(),
                },
            );
        }

        println!("✅ Market data initialized!");

        Self {
            execution_manager,
            coordinator,
            router,
            market_data: Arc::new(RwLock::new(market_data)),
            current_tick: Arc::new(Mutex::new(0)),
            active_symbols: Arc::new(RwLock::new((1..=10).collect())),
        }
    }

    /// Submit an order using the proper production architecture:
    /// OrderRouter → SymbolCoordinator → Whistle → ExecutionManager
    pub fn submit_order(
        &self,
        symbol_id: u32,
        account_id: AccountId,
        side: Side,
        price: Price,
        qty: Qty,
    ) -> Result<(), String> {
        println!("🔄 Submitting order: {:?} {} @ {} (Account: {})", side, qty, price, account_id);

        // Step 1: Ensure symbol is active and get queue writer
        println!("📋 Step 1: Ensuring symbol {} is active...", symbol_id);
        let ready_at_tick = {
            let coordinator = self.coordinator.lock().map_err(|_| "Failed to lock coordinator")?;
            coordinator
                .ensure_active(symbol_id)
                .map_err(|e| format!("Failed to activate symbol {symbol_id}: {:?}", e))?
        };
        println!("✅ Symbol {} activated, ready at tick {}", symbol_id, ready_at_tick.next_tick);

        // Step 2: Create order message
        println!("📝 Step 2: Creating order message...");
        let order_msg = InboundMsg::submit(
            0, // order_id - will be assigned by engine
            account_id,
            side,
            OrderType::Limit,
            Some(price),
            qty,
            0, // ts_norm
            0, // meta
            0, // enq_seq
        );

        // Step 3: Submit order via OrderRouter
        println!("📤 Step 3: Enqueuing order...");
        // TODO: Implement proper OrderRouter integration
        // For now, we'll use the coordinator's queue writer directly
        let mut queue_writer = ready_at_tick.queue_writer;
        queue_writer
            .try_enqueue(order_msg)
            .map_err(|e| format!("Failed to enqueue order: {:?}", e))?;
        println!("✅ Order enqueued successfully");

        // Step 4: Advance tick (order processing will be handled by SimulationClock when implemented)
        println!("⚙️ Step 4: Advancing tick...");
        let current_tick = self.advance_tick();
        println!("✅ Advanced to tick {}", current_tick);

        // TODO: Implement SimulationClock to process the enqueued order
        // For now, the order is successfully enqueued and waiting to be processed
        println!("📋 Order is now in the queue waiting for SimulationClock to process it");

        println!("🎉 Order submission completed successfully!");
        Ok(())
    }

    pub fn update_market_data(&self, symbol_id: u32, events: &[EngineEvent]) {
        let mut market_data_guard = self.market_data.write().unwrap();
        if let Some(market_data) = market_data_guard.get_mut(&symbol_id) {
            for event in events {
                match event {
                    EngineEvent::Trade(trade) => {
                        market_data.last_trade_price = Some(trade.price);
                        market_data.last_trade_qty = Some(trade.qty);
                        market_data.last_trade_time = Some(trade.tick);
                        market_data.trades.push(Trade {
                            price: trade.price,
                            qty: trade.qty,
                            side: trade.taker_side,
                            timestamp: trade.tick,
                            exec_id: trade.exec_id,
                        });
                    }
                    EngineEvent::BookDelta(delta) => {
                        market_data.book_deltas.push(BookDelta {
                            side: delta.side,
                            price: delta.price,
                            qty: delta.level_qty_after,
                            timestamp: delta.tick,
                        });

                        // Update best bid/ask
                        match delta.side {
                            Side::Buy => {
                                if delta.level_qty_after > 0 {
                                    market_data.bid_price = Some(delta.price);
                                    market_data.bid_qty = Some(delta.level_qty_after);
                                } else {
                                    market_data.bid_price = None;
                                    market_data.bid_qty = None;
                                }
                            }
                            Side::Sell => {
                                if delta.level_qty_after > 0 {
                                    market_data.ask_price = Some(delta.price);
                                    market_data.ask_qty = Some(delta.level_qty_after);
                                } else {
                                    market_data.ask_price = None;
                                    market_data.ask_qty = None;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn get_market_data(&self, symbol_id: u32) -> Option<MarketData> {
        let market_data_guard = self.market_data.read().unwrap();
        market_data_guard.get(&symbol_id).cloned()
    }

    pub fn advance_tick(&self) -> TickId {
        let mut tick_guard = self.current_tick.lock().unwrap();
        *tick_guard += 1;
        *tick_guard
    }

    pub fn get_current_tick(&self) -> TickId {
        *self.current_tick.lock().unwrap()
    }

    /// Get order book data for display purposes
    /// This ensures the symbol is activated and gets data from the actual trading engine
    pub fn get_order_book_data(
        &self,
        symbol_id: u32,
    ) -> Result<(Vec<(u32, u64)>, Vec<(u32, u64)>), String> {
        // Step 1: Ensure symbol is active (but don't process ticks yet)
        let coordinator = self.coordinator.lock().map_err(|_| "Failed to lock coordinator")?;
        let _ready_at_tick = coordinator
            .ensure_active(symbol_id)
            .map_err(|e| format!("Failed to activate symbol {symbol_id}: {:?}", e))?;

        // Step 2: Create a temporary engine with the same config for display
        // TODO: In the future, we should get this data from the actual engine
        let cfg = whistle::EngineCfg {
            symbol: symbol_id,
            price_domain: whistle::PriceDomain { floor: 100, ceil: 200, tick: 5 },
            bands: whistle::Bands { mode: whistle::BandMode::Percent(1000) },
            batch_max: 1024,
            arena_capacity: 4096,
            elastic_arena: false,
            exec_shift_bits: 12,
            exec_id_mode: whistle::ExecIdMode::Sharded,
            self_match_policy: whistle::SelfMatchPolicy::Skip,
            allow_market_cold_start: false,
            reference_price_source: whistle::ReferencePriceSource::SnapshotLastTrade,
        };

        let engine = whistle::Whistle::new(cfg);
        let asks = engine.get_order_book_levels(whistle::Side::Sell);
        let bids = engine.get_order_book_levels(whistle::Side::Buy);

        Ok((asks, bids))
    }
}

// Global system state instance
lazy_static::lazy_static! {
    pub static ref SYSTEM_STATE: SystemState = SystemState::new();
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Dashboard { update_ms: _ } => {
            run_dashboard_menu();
        }
        Commands::Interactive => {
            run_interactive();
        }
        Commands::Analytics { update_ms } => {
            run_analytics(update_ms);
        }
        Commands::Live { update_ms: _ } => {
            run_live_dashboard_menu();
        }
        Commands::Menu => {
            run_main_menu();
        }
    }
}

fn run_main_menu() {
    loop {
        println!("{}", "🚀 Waiver Exchange Admin CLI".cyan().bold());
        println!("===================================");
        println!();
        println!("📊 Available Modes:");
        println!("  1. {} - Real-time order book dashboard", "Dashboard".green().bold());
        println!("  2. {} - Interactive order submission", "Interactive".yellow().bold());
        println!("  3. {} - Analytics and metrics", "Analytics".blue().bold());
        println!("  4. {} - Live trading dashboard", "Live Dashboard".magenta().bold());
        println!("  5. {} - Exit", "Exit".red().bold());
        println!();

        print!("Select mode (1-5): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            match input.trim() {
                "1" => {
                    run_dashboard_menu();
                    break;
                }
                "2" => {
                    run_interactive();
                    break;
                }
                "3" => {
                    run_analytics(1000);
                    break;
                }
                "4" => {
                    run_live_dashboard_menu();
                    break;
                }
                "5" => {
                    println!("{}", "👋 Goodbye!".green());
                    break;
                }
                _ => {
                    println!("{}", "❌ Invalid selection. Please choose 1-5.".red());
                    println!();
                }
            }
        }
    }
}

fn run_dashboard_menu() {
    println!("{}", "📊 Dashboard Mode".green().bold());
    println!("==================");
    println!();
    println!("📈 Available Symbols");
    println!("  Symbols 1-10 are available");
    println!("  Enter symbol number (1-10) or 'back' to return to main menu");
    println!();

    loop {
        print!("Select symbol: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            match input.trim().to_lowercase().as_str() {
                "back" => {
                    run_main_menu();
                    return;
                }
                symbol_str => {
                    if let Ok(symbol_id) = symbol_str.parse::<u32>() {
                        if (1..=10).contains(&symbol_id) {
                            run_dashboard_single_symbol(symbol_id);
                            return;
                        } else {
                            println!("{}", "❌ Symbol must be between 1 and 10".red());
                        }
                    } else {
                        println!("{}", "❌ Invalid symbol number".red());
                    }
                }
            }
        }
        println!();
    }
}

fn run_dashboard_single_symbol(symbol_id: u32) {
    println!("{}", format!("🚀 Starting Symbol {symbol_id} Dashboard").cyan().bold());
    println!("Real-time order book monitoring for Symbol {symbol_id}");
    println!("Press Ctrl+C to exit completely");
    println!();

    let system_state = &SYSTEM_STATE;
    let mut last_tick = system_state.get_current_tick();
    let mut prompt_counter = 0;

    loop {
        let current_tick = system_state.get_current_tick();

        // Update header only if tick changed
        if current_tick != last_tick {
            update_single_symbol_header(system_state, symbol_id);
            display_single_symbol_content(system_state, symbol_id);
            last_tick = current_tick;
        }

        // Show prompt every 20 iterations
        prompt_counter += 1;
        if prompt_counter >= 20 {
            println!(
                "{}",
                "Press 'q' to go back to symbol selection, or Ctrl+C to exit completely".yellow()
            );
            prompt_counter = 0;
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}

fn update_single_symbol_header(system_state: &SystemState, _symbol_id: u32) {
    // Move cursor to the beginning of the header line and clear it
    print!("\r\x1B[K");

    // Update only the tick and time in the header
    print!(
        "  Tick: {} | Time: {}",
        system_state.get_current_tick(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    );

    // Flush the output to ensure it's displayed
    io::stdout().flush().unwrap();
}

fn display_single_symbol_content(system_state: &SystemState, symbol_id: u32) {
    if let Some(market_data) = system_state.get_market_data(symbol_id) {
        match system_state.get_order_book_data(symbol_id) {
            Ok((asks, bids)) => {
                display_order_book_from_data(&asks, &bids, &market_data);
            }
            Err(e) => {
                println!(
                    "{}",
                    format!("❌ Failed to get order book data for Symbol {symbol_id}: {}", e).red()
                );
            }
        }
    } else {
        println!("{}", format!("❌ Symbol {symbol_id} not found").red());
    }
}

fn display_order_book(engine: &whistle::Whistle, market_data: &MarketData) {
    println!("  📚 Order Book:");

    // Get full order book data from the engine
    let asks = engine.get_order_book_levels(whistle::Side::Sell); // Sell orders (asks)
    let bids = engine.get_order_book_levels(whistle::Side::Buy); // Buy orders (bids)

    display_order_book_from_data(&asks, &bids, market_data);
}

fn display_order_book_from_data(
    asks: &[(u32, u64)],
    bids: &[(u32, u64)],
    market_data: &MarketData,
) {
    println!("  📚 Order Book:");

    // Display top 10 asks (sells) - highest price first
    println!("    {} (Top 10 Sells)", "Price | Amount | Total".dimmed());
    for (price, qty) in asks.iter().rev().take(10) {
        // rev() to show highest first
        let total = (*price as u64) * qty;
        println!("    {} | {} | {}", price.to_string().red(), qty, total);
    }
    if asks.is_empty() {
        println!("    {}", "No sell orders".dimmed());
    }

    // Display last trade price in the middle
    if let Some(last_price) = market_data.last_trade_price {
        println!("    {} @ {}", "Last Trade:".bold(), last_price.to_string().yellow().bold());
    } else {
        println!("    {}", "Last Trade: None".dimmed());
    }

    // Display top 10 bids (buys) - highest price first
    println!("    {} (Top 10 Buys)", "Price | Amount | Total".dimmed());
    for (price, qty) in bids.iter().take(10) {
        // already sorted highest first
        let total = (*price as u64) * qty;
        println!("    {} | {} | {}", price.to_string().green(), qty, total);
    }
    if bids.is_empty() {
        println!("    {}", "No buy orders".dimmed());
    }

    println!();
}

fn run_live_dashboard_menu() {
    println!("{}", "🚀 Live Dashboard Mode".magenta().bold());
    println!("===================================");
    println!();
    println!("📊 Initializing order books...");

    let _system_state = &SYSTEM_STATE;

    println!("📈 Available Symbols");
    println!("  Symbols 1-10 are available");
    println!("  Enter symbol number (1-10) or 'back' to return to main menu");
    println!();

    loop {
        print!("Select symbol: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            match input.trim().to_lowercase().as_str() {
                "back" => {
                    run_main_menu();
                    return;
                }
                symbol_str => {
                    if let Ok(symbol_id) = symbol_str.parse::<u32>() {
                        if (1..=10).contains(&symbol_id) {
                            run_live_dashboard(symbol_id, 100);
                            return;
                        } else {
                            println!("{}", "❌ Symbol must be between 1 and 10".red());
                        }
                    } else {
                        println!("{}", "❌ Invalid symbol number".red());
                    }
                }
            }
        }
        println!();
    }
}

fn run_live_dashboard(symbol_id: u32, _update_ms: u64) {
    println!("{}", format!("🚀 Live Trading Dashboard - Symbol {symbol_id}").cyan().bold());
    println!("  Real-time order book + order submission");
    println!("  Type commands and press Enter");
    println!("  Type 'back' to return to symbol selection");
    println!();

    let system_state = &SYSTEM_STATE;

    // Initial display - show order book once
    display_live_dashboard_content(system_state, symbol_id);
    show_live_commands();

    // Simple input loop without threading
    loop {
        print!("\nlive> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                let input = input.trim().to_lowercase();
                if input == "back" || input == "exit" {
                    println!("{}", "🔙 Returning to symbol selection...".yellow());
                    break;
                } else if input == "help" {
                    show_live_commands();
                } else if !input.is_empty() {
                    println!("🔄 Processing command: {}", input);
                    handle_live_command(system_state, symbol_id, &input);
                    // Refresh the display after each command
                    println!(); // Add spacing
                    display_live_dashboard_content(system_state, symbol_id);
                }
            }
            Err(e) => {
                println!("❌ Input error: {}", e);
                break;
            }
        }
    }
}

fn display_live_dashboard_content(system_state: &SystemState, symbol_id: u32) {
    if let Some(market_data) = system_state.get_market_data(symbol_id) {
        match system_state.get_order_book_data(symbol_id) {
            Ok((asks, bids)) => {
                display_order_book_from_data(&asks, &bids, &market_data);
            }
            Err(e) => {
                println!(
                    "{}",
                    format!("❌ Failed to get order book data for Symbol {symbol_id}: {}", e).red()
                );
                // Fallback: show empty order book
                display_order_book_from_data(&[], &[], &market_data);
            }
        }
    } else {
        println!("{}", format!("❌ Symbol {symbol_id} not found").red());
    }
}

fn update_order_book_in_place(system_state: &SystemState, symbol_id: u32) {
    // Clear the screen and move cursor to top
    print!("\x1B[2J\x1B[1;1H");

    // Redraw the header
    println!("{}", format!("🚀 Live Trading Dashboard - Symbol {symbol_id}").cyan().bold());
    println!("  Real-time order book + order submission");
    println!("  Type commands in the background (no prompts) or 'help' for commands");
    println!("  Press Ctrl+C to exit");
    println!();

    // Redraw the order book content
    display_live_dashboard_content(system_state, symbol_id);

    // Redraw the commands
    show_live_commands();

    // Flush the output
    io::stdout().flush().unwrap();
}

fn show_live_commands() {
    println!();
    println!("{}", "💡 Available Commands:".yellow().bold());
    println!("  submit <Buy/Sell> <price> <qty> <account_id> - Submit an order");
    println!("  tick                                         - Advance the system tick");
    println!("  status                                       - Show current system status");
    println!("  help                                         - Show this help message");
    println!("  back/exit                                    - Return to symbol selection");
    println!();
}

fn handle_live_command(system_state: &SystemState, symbol_id: u32, input: &str) {
    let parts: Vec<&str> = input.split_whitespace().collect();

    match parts[0] {
        "submit" => {
            if parts.len() >= 5 {
                handle_live_submit_order(system_state, symbol_id, parts);
            } else {
                println!("❌ Usage: submit <Buy/Sell> <price> <qty> <account_id>");
            }
        }
        "tick" => {
            let new_tick = system_state.advance_tick();
            println!("⏰ Advanced tick to {new_tick}");
        }
        "status" => {
            handle_show_status(system_state);
        }
        _ => println!("❌ Unknown command. Type 'help' for available commands."),
    }
}

fn handle_live_submit_order(system_state: &SystemState, symbol_id: u32, parts: Vec<&str>) {
    let side = match parts[1].to_lowercase().as_str() {
        "buy" => Side::Buy,
        "sell" => Side::Sell,
        _ => {
            println!("❌ Invalid side. Use 'Buy' or 'Sell'");
            return;
        }
    };

    let price: Price = parts[2].parse().unwrap_or_else(|_| {
        println!("❌ Invalid price");
        0
    });
    let qty: Qty = parts[3].parse().unwrap_or_else(|_| {
        println!("❌ Invalid quantity");
        0
    });
    let account_id: AccountId = parts[4].parse().unwrap_or_else(|_| {
        println!("❌ Invalid account ID");
        0
    });

    if price == 0 || qty == 0 || account_id == 0 {
        return;
    }

    if let Err(e) = system_state.submit_order(symbol_id, account_id, side, price, qty) {
        println!("❌ Failed to submit order: {e}");
    } else {
        println!("✅ Order submitted: {side:?} {qty} @ {price} (Account: {account_id})");
        // Update the order book in place to show the new order
        update_order_book_in_place(system_state, symbol_id);
    }
}

fn run_interactive() {
    println!("{}", "🎮 Interactive Mode".yellow().bold());
    println!("=====================");
    println!();
    println!("Type commands to interact with the system. Type 'help' for available commands.");
    println!();

    let system_state = &SYSTEM_STATE;

    loop {
        print!("admin-cli> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let input = input.trim();
            if input == "exit" {
                println!("{}", "👋 Goodbye!".green());
                break;
            }
            handle_interactive_command(system_state, input);
        }
    }
}

fn handle_interactive_command(system_state: &SystemState, input: &str) {
    let parts: Vec<&str> = input.split_whitespace().collect();

    if parts.is_empty() {
        return;
    }

    match parts[0] {
        "submit" => {
            if parts.len() >= 6 {
                handle_submit_order(system_state, parts);
            } else {
                println!("❌ Usage: submit <symbol_id> <Buy/Sell> <price> <qty> <account_id>");
            }
        }
        "tick" => {
            handle_advance_tick(system_state);
        }
        "status" => {
            handle_show_status(system_state);
        }
        "book" => {
            if parts.len() >= 2 {
                handle_show_order_book(system_state, parts);
            } else {
                println!("❌ Usage: book <symbol_id>");
            }
        }
        "help" => {
            show_interactive_help();
        }
        _ => println!("❌ Unknown command. Type 'help' for available commands."),
    }
}

fn handle_submit_order(system_state: &SystemState, parts: Vec<&str>) {
    let symbol_id: u32 = parts[1].parse().unwrap_or_else(|_| {
        println!("❌ Invalid symbol ID");
        0
    });
    if symbol_id == 0 {
        return;
    }
    let side = match parts[2].to_lowercase().as_str() {
        "buy" => Side::Buy,
        "sell" => Side::Sell,
        _ => {
            println!("❌ Invalid side. Use 'Buy' or 'Sell'");
            return;
        }
    };
    let price: Price = parts[3].parse().unwrap_or_else(|_| {
        println!("❌ Invalid price");
        0
    });
    let qty: Qty = parts[4].parse().unwrap_or_else(|_| {
        println!("❌ Invalid quantity");
        0
    });
    let account_id: AccountId = parts[5].parse().unwrap_or_else(|_| {
        println!("❌ Invalid account ID");
        0
    });

    if price == 0 || qty == 0 || account_id == 0 {
        return;
    }

    if let Err(e) = system_state.submit_order(symbol_id, account_id, side, price, qty) {
        println!("❌ Failed to submit order: {e}");
    } else {
        println!(
            "✅ Order submitted: {side:?} {qty} {symbol_id} @ {price} (Account: {account_id})"
        );
    }
}

fn handle_advance_tick(system_state: &SystemState) {
    let new_tick = system_state.advance_tick();
    println!("⏰ Advanced tick to {new_tick}");
}

fn handle_show_status(system_state: &SystemState) {
    println!("📊 System Status:");
    println!("  Current Tick: {}", system_state.get_current_tick());
    println!("  Active Symbols: {:?}", system_state.active_symbols.read().unwrap());
    println!("---------------------\n");
}

fn handle_show_order_book(system_state: &SystemState, parts: Vec<&str>) {
    let symbol_id: u32 = parts[1].parse().unwrap_or_else(|_| {
        println!("❌ Invalid symbol ID");
        0
    });
    if symbol_id == 0 {
        return;
    }

    if let Some(market_data) = system_state.get_market_data(symbol_id) {
        match system_state.get_order_book_data(symbol_id) {
            Ok((asks, bids)) => {
                display_order_book_from_data(&asks, &bids, &market_data);
            }
            Err(e) => {
                println!("❌ Failed to get order book data for Symbol {symbol_id}: {}", e);
            }
        }
    } else {
        println!("❌ No market data found for Symbol {symbol_id}");
    }
}

fn show_interactive_help() {
    println!("{}", "📚 Available Commands:".cyan().bold());
    println!("  submit <symbol_id> <Buy/Sell> <price> <qty> <account_id> - Submit an order");
    println!(
        "  tick                                                      - Advance the system tick"
    );
    println!(
        "  status                                                    - Show current system status"
    );
    println!(
        "  book <symbol_id>                                          - Show order book for symbol"
    );
    println!(
        "  {}                                                 - Exit interactive mode",
        "exit".green()
    );
    println!();
}

fn run_analytics(update_ms: u64) {
    println!("{}", "📊 Analytics Mode".blue().bold());
    println!("===================");
    println!();
    println!("Real-time analytics and metrics display");
    println!("Press Ctrl+C to exit");
    println!();

    let system_state = &SYSTEM_STATE;
    let update_duration = Duration::from_millis(update_ms);
    let mut last_update = Instant::now();

    loop {
        if last_update.elapsed() >= update_duration {
            display_analytics(system_state);
            last_update = Instant::now();
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}

fn display_analytics(system_state: &SystemState) {
    // Clear screen and move cursor to top
    print!("\x1B[2J\x1B[1;1H");

    println!("{}", "📊 Waiver Exchange Analytics".blue().bold());
    println!("=================================");
    println!();

    println!("⏰ System Status:");
    println!("  Current Tick: {}", system_state.get_current_tick());
    println!("  Active Symbols: {:?}", system_state.active_symbols.read().unwrap());
    println!();

    println!("📈 Market Data Summary:");
    let market_data_guard = system_state.market_data.read().unwrap();
    for (symbol_id, market_data) in market_data_guard.iter() {
        if market_data.last_trade_price.is_some()
            || market_data.bid_price.is_some()
            || market_data.ask_price.is_some()
        {
            println!(
                "  Symbol {}: Last Trade: {:?}, Bid: {:?}, Ask: {:?}",
                symbol_id,
                market_data.last_trade_price,
                market_data.bid_price,
                market_data.ask_price
            );
        }
    }
    println!();

    println!("🔍 Performance Metrics:");
    println!("  Total Orders Processed: [Calculating...]");
    println!("  Average Latency: [Calculating...]");
    println!("  Throughput: [Calculating...]");
    println!();

    println!("🚨 Alerts and notifications will appear here");
    println!();
}
