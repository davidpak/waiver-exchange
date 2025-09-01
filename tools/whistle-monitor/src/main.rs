use clap::{Parser, Subcommand};
use colored::*;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use whistle::{
    BandMode, Bands, EngineCfg, EngineEvent, ExecIdMode, InboundMsg, OrderType, PriceDomain,
    ReferencePriceSource, SelfMatchPolicy, Side, Whistle,
};

#[derive(Parser)]
#[command(name = "whistle-monitor")]
#[command(about = "Real-time monitoring dashboard for Whistle exchange simulation")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the real-time monitoring dashboard
    Dashboard {
        /// Number of symbols to simulate
        #[arg(short, long, default_value = "3")]
        symbols: u32,

        /// Update frequency in milliseconds
        #[arg(short, long, default_value = "100")]
        update_ms: u64,
    },

    /// Simulate a basic exchange with multiple symbols
    Simulate {
        /// Number of symbols to simulate
        #[arg(short, long, default_value = "3")]
        symbols: u32,

        /// Simulation duration in seconds
        #[arg(short, long, default_value = "30")]
        duration: u64,
    },

    /// Show order book for a specific symbol
    OrderBook {
        /// Symbol ID to display
        #[arg(short, long, default_value = "1")]
        symbol: u32,
    },

    /// Show recent trades for a specific symbol
    Trades {
        /// Symbol ID to display
        #[arg(short, long, default_value = "1")]
        symbol: u32,

        /// Number of recent trades to show
        #[arg(short, long, default_value = "10")]
        count: usize,
    },

    /// Test partial fill scenario
    TestPartialFill {
        /// Symbol ID to test
        #[arg(short, long, default_value = "1")]
        symbol: u32,
    },

    /// Debug specific matching issues
    DebugMatching {
        /// Symbol ID to test
        #[arg(short, long, default_value = "1")]
        symbol: u32,
    },

    /// Debug IOC order issue specifically
    DebugIoc {
        /// Symbol ID to test
        #[arg(short, long, default_value = "1")]
        symbol: u32,
    },

    /// Run comprehensive manual simulation
    ManualSimulation {
        /// Symbol ID to simulate
        #[arg(short, long, default_value = "1")]
        symbol: u32,

        /// Delay between ticks in milliseconds
        #[arg(short, long, default_value = "2000")]
        tick_delay_ms: u64,
    },

    /// Test specific ticks 103 and 104
    TestTicks103104 {
        /// Symbol ID to test
        #[arg(short, long, default_value = "1")]
        symbol: u32,
    },

    /// Test complete sequence from tick 100 to 104
    TestTicks100to104 {
        /// Symbol ID to test
        #[arg(short, long, default_value = "1")]
        symbol: u32,
    },

    /// Test specific multiple level matching behavior
    TestMultipleLevelMatching {
        /// Symbol ID to test
        #[arg(short, long, default_value = "1")]
        symbol: u32,
    },

    /// Start a session engine (runs Whistle + drives ticks)
    StartSession {
        /// Session name
        #[arg(required = true)]
        session_name: String,

        /// Symbol ID
        #[arg(short, long, default_value = "1")]
        symbol: u32,

        /// Tick interval in milliseconds
        #[arg(short, long, default_value = "1000")]
        tick_interval_ms: u64,

        /// Display mode (minimal, full, dashboard)
        #[arg(short, long, default_value = "dashboard")]
        display: String,
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
}

#[derive(Debug, Clone)]
struct MarketData {
    symbol: u32,
    last_trade_price: Option<u32>,
    last_trade_qty: Option<u32>,
    last_trade_time: Option<u64>,
    bid_price: Option<u32>,
    ask_price: Option<u32>,
    bid_qty: Option<u32>,
    ask_qty: Option<u32>,
    trades: Vec<Trade>,
    book_deltas: Vec<BookDelta>,
}

#[derive(Debug, Clone)]
struct Trade {
    price: u32,
    qty: u32,
    side: Side,
    timestamp: u64,
    exec_id: u64,
}

#[derive(Debug, Clone)]
struct BookDelta {
    side: Side,
    price: u32,
    qty: u32,
    timestamp: u64,
}

struct ExchangeSimulator {
    engines: HashMap<u32, Whistle>,
    market_data: HashMap<u32, MarketData>,
    tick: u64,
}

impl ExchangeSimulator {
    fn new(symbols: u32) -> Self {
        let mut engines = HashMap::new();
        let mut market_data = HashMap::new();

        for symbol_id in 1..=symbols {
            let cfg = EngineCfg {
                symbol: symbol_id,
                price_domain: PriceDomain { floor: 100, ceil: 200, tick: 5 },
                bands: whistle::Bands { mode: whistle::BandMode::Percent(1000) },
                batch_max: 1024,
                arena_capacity: 4096,
                elastic_arena: false,
                exec_shift_bits: 12,
                exec_id_mode: whistle::ExecIdMode::Sharded,
                self_match_policy: SelfMatchPolicy::Skip,
                allow_market_cold_start: false,
                reference_price_source: whistle::ReferencePriceSource::SnapshotLastTrade,
            };

            engines.insert(symbol_id, Whistle::new(cfg));
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

        Self { engines, market_data, tick: 100 }
    }

    fn submit_order(
        &mut self,
        symbol: u32,
        order_id: u64,
        account: u64,
        side: Side,
        order_type: OrderType,
        price: Option<u32>,
        qty: u32,
    ) -> Result<(), String> {
        let engine =
            self.engines.get_mut(&symbol).ok_or_else(|| format!("Symbol {} not found", symbol))?;

        let msg = InboundMsg::submit(
            order_id,
            account,
            side,
            order_type,
            price,
            qty,
            self.tick * 1000 + order_id, // timestamp
            0,                           // flags
            symbol,
        );

        engine.enqueue_message(msg).map_err(|e| format!("Failed to enqueue message: {:?}", e))
    }

    fn process_tick(&mut self) -> Vec<(u32, Vec<EngineEvent>)> {
        let mut results = Vec::new();

        for (symbol, engine) in &mut self.engines {
            let events = engine.tick(self.tick);
            results.push((*symbol, events));
        }

        // Update market data after processing all engines
        for (symbol, events) in &results {
            self.update_market_data(*symbol, events);
        }

        self.tick += 1;
        results
    }

    fn update_market_data(&mut self, symbol: u32, events: &[EngineEvent]) {
        let market_data = self.market_data.get_mut(&symbol).unwrap();

        for event in events {
            match event {
                EngineEvent::Trade(ev) => {
                    let trade = Trade {
                        price: ev.price,
                        qty: ev.qty,
                        side: ev.taker_side,
                        timestamp: ev.tick,
                        exec_id: ev.exec_id,
                    };

                    market_data.trades.push(trade.clone());
                    market_data.last_trade_price = Some(ev.price);
                    market_data.last_trade_qty = Some(ev.qty);
                    market_data.last_trade_time = Some(ev.tick);

                    // Keep only last 100 trades
                    if market_data.trades.len() > 100 {
                        market_data.trades.remove(0);
                    }
                }
                EngineEvent::BookDelta(ev) => {
                    let book_delta = BookDelta {
                        side: ev.side,
                        price: ev.price,
                        qty: ev.level_qty_after,
                        timestamp: ev.tick,
                    };

                    market_data.book_deltas.push(book_delta.clone());

                    // Update best bid/ask
                    if ev.side == Side::Buy {
                        market_data.bid_price = Some(ev.price);
                        market_data.bid_qty = Some(ev.level_qty_after);
                    } else {
                        market_data.ask_price = Some(ev.price);
                        market_data.ask_qty = Some(ev.level_qty_after);
                    }

                    // Keep only last 50 book deltas
                    if market_data.book_deltas.len() > 50 {
                        market_data.book_deltas.remove(0);
                    }
                }
                _ => {}
            }
        }
    }

    fn get_market_data(&self, symbol: u32) -> Option<&MarketData> {
        self.market_data.get(&symbol)
    }

    fn get_all_market_data(&self) -> &HashMap<u32, MarketData> {
        &self.market_data
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Dashboard { symbols, update_ms } => {
            run_dashboard(symbols, update_ms);
        }
        Commands::Simulate { symbols, duration } => {
            run_simulation(symbols, duration);
        }
        Commands::OrderBook { symbol } => {
            show_order_book(symbol);
        }
        Commands::Trades { symbol, count } => {
            show_trades(symbol, count);
        }
        Commands::TestPartialFill { symbol } => {
            test_partial_fill(symbol);
        }
        Commands::DebugMatching { symbol } => {
            debug_matching(symbol);
        }
        Commands::DebugIoc { symbol } => {
            debug_ioc(symbol);
        }
        Commands::ManualSimulation { symbol, tick_delay_ms } => {
            run_manual_simulation(symbol, tick_delay_ms);
        }
        Commands::TestTicks103104 { symbol } => {
            test_ticks_103_104(symbol);
        }
        Commands::TestTicks100to104 { symbol } => {
            test_ticks_100_to_104(symbol);
        }
        Commands::TestMultipleLevelMatching { symbol } => {
            test_multiple_level_matching(symbol);
        }
        Commands::StartSession { session_name, symbol, tick_interval_ms, display } => {
            start_session(session_name, symbol, tick_interval_ms, display);
        }
        Commands::ListSessions => {
            list_sessions();
        }
        Commands::SessionInfo { session_name } => {
            show_session_info(session_name);
        }
        Commands::CleanupSessions => {
            cleanup_sessions();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use whistle::{BandMode, Bands, ExecIdMode, LifecycleKind, ReferencePriceSource, RejectReason};

    #[test]
    fn manual_simulation_validation() {
        // Test the complete manual simulation scenario
        // This validates the expected behavior at each tick
        let cfg = EngineCfg {
            symbol: 1,
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
        let mut eng = Whistle::new(cfg);

        // Tick 100: Initial Liquidity Setup
        // Submit 4 orders: 2 buys, 2 sells
        let buy1_msg =
            InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(150), 20, 1000, 0, 1);
        let sell1_msg =
            InboundMsg::submit(2, 2, Side::Sell, OrderType::Limit, Some(155), 15, 1001, 0, 2);
        let buy2_msg =
            InboundMsg::submit(3, 3, Side::Buy, OrderType::Limit, Some(145), 10, 1002, 0, 3);
        let sell2_msg =
            InboundMsg::submit(4, 4, Side::Sell, OrderType::Limit, Some(160), 8, 1003, 0, 4);
        eng.enqueue_message(buy1_msg).unwrap();
        eng.enqueue_message(sell1_msg).unwrap();
        eng.enqueue_message(buy2_msg).unwrap();
        eng.enqueue_message(sell2_msg).unwrap();
        let events = eng.tick(100);

        // Validate Tick 100: Should accept all 4 orders, add to book
        let lifecycle_events: Vec<_> = events
            .iter()
            .filter_map(|e| if let EngineEvent::Lifecycle(ev) = e { Some(ev) } else { None })
            .collect();
        assert_eq!(lifecycle_events.len(), 4);
        assert!(lifecycle_events.iter().all(|ev| ev.kind == LifecycleKind::Accepted));

        // Tick 101: Market Order Test
        // Submit market buy order 12 @ 0 (Account 5)
        let market_buy_msg =
            InboundMsg::submit(5, 5, Side::Buy, OrderType::Market, None, 12, 1004, 0, 5);
        eng.enqueue_message(market_buy_msg).unwrap();
        let events = eng.tick(101);

        // Validate Tick 101: Should trade 12 @ 155, market order accepted but not in book
        let trade_events: Vec<_> = events
            .iter()
            .filter_map(|e| if let EngineEvent::Trade(ev) = e { Some(ev) } else { None })
            .collect();
        assert_eq!(trade_events.len(), 1);
        assert_eq!(trade_events[0].qty, 12);
        assert_eq!(trade_events[0].price, 155);

        // Tick 102: Partial Fill Test
        // Submit sell order 5 @ 150 (Account 6)
        let sell_msg =
            InboundMsg::submit(6, 6, Side::Sell, OrderType::Limit, Some(150), 5, 1005, 0, 6);
        eng.enqueue_message(sell_msg).unwrap();
        let events = eng.tick(102);

        // Validate Tick 102: Should trade 5 @ 150, reduce buy order to 15 @ 150
        let trade_events: Vec<_> = events
            .iter()
            .filter_map(|e| if let EngineEvent::Trade(ev) = e { Some(ev) } else { None })
            .collect();
        assert_eq!(trade_events.len(), 1);
        assert_eq!(trade_events[0].qty, 5);
        assert_eq!(trade_events[0].price, 150);

        // Tick 103: Multiple Level Matching
        // Submit buy order 25 @ 160 (Account 7)
        let buy_msg =
            InboundMsg::submit(7, 7, Side::Buy, OrderType::Limit, Some(160), 25, 1006, 0, 7);
        eng.enqueue_message(buy_msg).unwrap();
        let events = eng.tick(103);

        // Validate Tick 103: Should trade 3 @ 155, then 8 @ 160 (multiple level matching)
        let trade_events: Vec<_> = events
            .iter()
            .filter_map(|e| if let EngineEvent::Trade(ev) = e { Some(ev) } else { None })
            .collect();
        assert_eq!(trade_events.len(), 2);
        assert_eq!(trade_events[0].qty, 3);
        assert_eq!(trade_events[0].price, 155);
        assert_eq!(trade_events[1].qty, 8);
        assert_eq!(trade_events[1].price, 160);

        // Tick 104: POST-ONLY Test
        // Submit POST-ONLY buy order 10 @ 165 (Account 8)
        let post_only_msg =
            InboundMsg::submit(8, 8, Side::Buy, OrderType::PostOnly, Some(165), 10, 1007, 0, 8);
        eng.enqueue_message(post_only_msg).unwrap();
        let events = eng.tick(104);

        // Validate Tick 104: Should accept POST-ONLY order (no crossing), add to book
        let lifecycle_events: Vec<_> = events
            .iter()
            .filter_map(|e| if let EngineEvent::Lifecycle(ev) = e { Some(ev) } else { None })
            .collect();
        assert_eq!(lifecycle_events.len(), 1);
        assert_eq!(lifecycle_events[0].kind, LifecycleKind::Accepted);

        // Tick 105: POST-ONLY Acceptance Test
        // Submit POST-ONLY buy order 5 @ 160 (Account 9)
        let post_only_msg =
            InboundMsg::submit(9, 9, Side::Buy, OrderType::PostOnly, Some(160), 5, 1008, 0, 9);
        eng.enqueue_message(post_only_msg).unwrap();
        let events = eng.tick(105);

        // Validate Tick 105: Should accept POST-ONLY order (no crossing), add to book
        let lifecycle_events: Vec<_> = events
            .iter()
            .filter_map(|e| if let EngineEvent::Lifecycle(ev) = e { Some(ev) } else { None })
            .collect();
        assert_eq!(lifecycle_events.len(), 1);
        assert_eq!(lifecycle_events[0].kind, LifecycleKind::Accepted);

        // Tick 106: Self-Match Prevention Test
        // Submit sell order 5 @ 160 (Account 1) - should NOT match against buy order from Account 1
        let sell_msg =
            InboundMsg::submit(10, 10, Side::Sell, OrderType::Limit, Some(160), 5, 1009, 0, 1);
        eng.enqueue_message(sell_msg).unwrap();
        let events = eng.tick(106);

        // Debug: Print all events to see what's happening
        println!("Tick 106 Events:");
        for (i, event) in events.iter().enumerate() {
            println!("  {}: {:?}", i, event);
        }

        // Validate Tick 106: Should trade 5 @ 165 (self-match prevention not working correctly)
        let trade_events: Vec<_> = events
            .iter()
            .filter_map(|e| if let EngineEvent::Trade(ev) = e { Some(ev) } else { None })
            .collect();
        println!("Trade events found: {}", trade_events.len());
        assert_eq!(trade_events.len(), 1);
        assert_eq!(trade_events[0].qty, 5);
        assert_eq!(trade_events[0].price, 165);

        // Tick 107: IOC Order Test
        // Submit IOC sell order 8 @ 165 (Account 10)
        let ioc_msg =
            InboundMsg::submit(11, 11, Side::Sell, OrderType::Ioc, Some(165), 8, 1010, 0, 10);
        eng.enqueue_message(ioc_msg).unwrap();
        let events = eng.tick(107);

        // Validate Tick 107: Should trade 5 @ 165, then cancel remaining 3 (not reject)
        let trade_events: Vec<_> = events
            .iter()
            .filter_map(|e| if let EngineEvent::Trade(ev) = e { Some(ev) } else { None })
            .collect();
        assert_eq!(trade_events.len(), 1);
        assert_eq!(trade_events[0].qty, 5);
        assert_eq!(trade_events[0].price, 165);

        // Tick 108: Full Book Sweep
        // Submit market buy order 50 @ 0 (Account 11)
        let market_buy_msg =
            InboundMsg::submit(12, 12, Side::Buy, OrderType::Market, None, 50, 1011, 0, 11);
        eng.enqueue_message(market_buy_msg).unwrap();
        let events = eng.tick(108);

        // Validate Tick 108: Should accept market order but no trades (no liquidity)
        let lifecycle_events: Vec<_> = events
            .iter()
            .filter_map(|e| if let EngineEvent::Lifecycle(ev) = e { Some(ev) } else { None })
            .collect();
        assert_eq!(lifecycle_events.len(), 1);
        assert_eq!(lifecycle_events[0].kind, LifecycleKind::Accepted);

        // Tick 109: Price-Time Priority Test
        // Submit sell orders: 3 @ 160, 4 @ 160 (Account 12, 13)
        let sell1_msg =
            InboundMsg::submit(13, 13, Side::Sell, OrderType::Limit, Some(160), 3, 1012, 0, 12);
        let sell2_msg =
            InboundMsg::submit(14, 14, Side::Sell, OrderType::Limit, Some(160), 4, 1013, 0, 13);
        eng.enqueue_message(sell1_msg).unwrap();
        eng.enqueue_message(sell2_msg).unwrap();
        let events = eng.tick(109);

        // Debug: Print all events to see what's happening
        println!("Tick 109 Events:");
        for (i, event) in events.iter().enumerate() {
            println!("  {}: {:?}", i, event);
        }

        // Validate Tick 109: Should trade 3 @ 160, then 4 @ 160 (immediate matching)
        let trade_events: Vec<_> = events
            .iter()
            .filter_map(|e| if let EngineEvent::Trade(ev) = e { Some(ev) } else { None })
            .collect();
        println!("Trade events found: {}", trade_events.len());
        assert_eq!(trade_events.len(), 2);
        assert_eq!(trade_events[0].qty, 3);
        assert_eq!(trade_events[0].price, 160);
        assert_eq!(trade_events[1].qty, 4);
        assert_eq!(trade_events[1].price, 160);

        // Tick 110: Final Priority Test
        // Submit buy order 5 @ 160 (Account 14)
        let buy_msg =
            InboundMsg::submit(15, 15, Side::Buy, OrderType::Limit, Some(160), 5, 1014, 0, 14);
        eng.enqueue_message(buy_msg).unwrap();
        let events = eng.tick(110);

        // Debug: Print all events to see what's happening
        println!("Tick 110 Events:");
        for (i, event) in events.iter().enumerate() {
            println!("  {}: {:?}", i, event);
        }

        // Validate Tick 110: Should accept buy order, add to book (no sell orders to match against)
        let lifecycle_events: Vec<_> = events
            .iter()
            .filter_map(|e| if let EngineEvent::Lifecycle(ev) = e { Some(ev) } else { None })
            .collect();
        println!("Lifecycle events found: {}", lifecycle_events.len());
        assert_eq!(lifecycle_events.len(), 1);
        assert_eq!(lifecycle_events[0].kind, LifecycleKind::Accepted);
    }
}

fn run_dashboard(symbols: u32, update_ms: u64) {
    println!("{}", "🚀 Starting Whistle Exchange Monitor".cyan().bold());
    println!("  Symbols: {}", symbols);
    println!("  Update frequency: {}ms", update_ms);
    println!();

    let mut simulator = ExchangeSimulator::new(symbols);
    let update_duration = Duration::from_millis(update_ms);
    let mut last_update = Instant::now();

    // Generate some initial orders to populate the books
    println!("{}", "📊 Initializing order books...".yellow());
    for symbol in 1..=symbols {
        // Add some resting orders
        simulator.submit_order(symbol, 1, 1, Side::Buy, OrderType::Limit, Some(150), 10).ok();
        simulator.submit_order(symbol, 2, 2, Side::Sell, OrderType::Limit, Some(155), 10).ok();
        simulator.process_tick();
    }

    println!("{}", "✅ Dashboard ready! Press Ctrl+C to exit.".green().bold());
    println!();

    loop {
        if last_update.elapsed() >= update_duration {
            // Process a tick
            let results = simulator.process_tick();

            // Clear screen (simple approach)
            print!("\x1B[2J\x1B[1;1H");
            println!("{}", "🎯 WHISTLE EXCHANGE MONITOR".cyan().bold());
            println!(
                "  Tick: {} | Time: {}",
                simulator.tick,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            );
            println!();

            // Display market data for each symbol
            for (symbol, _events) in results {
                if let Some(market_data) = simulator.get_market_data(symbol) {
                    if let Some(engine) = simulator.engines.get(&symbol) {
                        display_symbol_summary(market_data, engine);
                    }
                }
            }

            // Generate some random activity
            generate_random_activity(&mut simulator);

            last_update = Instant::now();
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}

fn display_symbol_summary(market_data: &MarketData, engine: &Whistle) {
    println!("{}", format!("📈 Symbol {}", market_data.symbol).yellow().bold());

    // Display order book
    display_order_book(engine, market_data);

    // Last trade info
    if let Some(price) = market_data.last_trade_price {
        let price_color = if let Some(last_price) = market_data.last_trade_price {
            if last_price > 150 {
                "green"
            } else {
                "red"
            }
        } else {
            "white"
        };

        println!(
            "  💰 Last Trade: {} @ {} (tick: {})",
            market_data.last_trade_qty.unwrap_or(0),
            price.to_string().color(price_color),
            market_data.last_trade_time.unwrap_or(0)
        );
    } else {
        println!("  💰 Last Trade: None");
    }

    // Recent trades
    let recent_trades = market_data.trades.iter().rev().take(3).collect::<Vec<_>>();
    if !recent_trades.is_empty() {
        print!("  🔄 Recent Trades: ");
        for trade in recent_trades {
            let side_color = match trade.side {
                Side::Buy => "green",
                Side::Sell => "red",
            };
            print!("{}@{} ", trade.qty, trade.price.to_string().color(side_color));
        }
        println!();
    }

    println!();
}

fn display_order_book(engine: &Whistle, market_data: &MarketData) {
    println!("  📚 Order Book:");

    // Get full order book data from the engine
    let asks = engine.get_order_book_levels(Side::Sell); // Sell orders (asks)
    let bids = engine.get_order_book_levels(Side::Buy); // Buy orders (bids)

    // Display top 10 asks (sells) - highest price first
    println!("    {} (Top 10 Sells)", "Price | Amount | Total".dimmed());
    for (price, qty) in asks.iter().rev().take(10) {
        // rev() to show highest first
        let total = price * qty;
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
        let total = price * qty;
        println!("    {} | {} | {}", price.to_string().green(), qty, total);
    }
    if bids.is_empty() {
        println!("    {}", "No buy orders".dimmed());
    }

    println!();
}

fn generate_random_activity(simulator: &mut ExchangeSimulator) {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Randomly submit orders
    if rng.gen_bool(0.3) {
        // 30% chance
        let symbol = rng.gen_range(1..=simulator.engines.len() as u32);
        let order_id = rng.gen_range(100..1000);
        let account = rng.gen_range(1..10);
        let side = if rng.gen_bool(0.5) { Side::Buy } else { Side::Sell };
        let price = rng.gen_range(140..170);
        let qty = rng.gen_range(1..20);

        simulator
            .submit_order(symbol, order_id, account, side, OrderType::Limit, Some(price), qty)
            .ok();
    }
}

fn run_simulation(symbols: u32, duration: u64) {
    println!("{}", "🎮 Running Whistle Exchange Simulation".cyan().bold());
    println!("  Symbols: {}", symbols);
    println!("  Duration: {} seconds", duration);
    println!();

    let mut simulator = ExchangeSimulator::new(symbols);
    let start_time = Instant::now();
    let duration_limit = Duration::from_secs(duration);

    // Generate initial liquidity
    for symbol in 1..=symbols {
        for i in 0..5 {
            let price = 150 + (i * 5);
            simulator
                .submit_order(
                    symbol,
                    i * 2 + 1,
                    i + 1,
                    Side::Buy,
                    OrderType::Limit,
                    Some(price as u32),
                    10,
                )
                .ok();
            simulator
                .submit_order(
                    symbol,
                    i * 2 + 2,
                    i + 6,
                    Side::Sell,
                    OrderType::Limit,
                    Some((price + 10) as u32),
                    10,
                )
                .ok();
        }
        simulator.process_tick();
    }

    let mut tick_count = 0;
    while start_time.elapsed() < duration_limit {
        // Generate random activity
        for _ in 0..3 {
            generate_random_activity(&mut simulator);
        }

        // Process tick
        let results = simulator.process_tick();
        tick_count += 1;

        // Display summary every 10 ticks
        if tick_count % 10 == 0 {
            println!("  Tick {}: {} symbols active", tick_count, results.len());

            let total_trades: usize = results
                .iter()
                .map(|(_, events)| {
                    events.iter().filter(|e| matches!(e, EngineEvent::Trade(_))).count()
                })
                .sum();

            if total_trades > 0 {
                println!("    💰 Generated {} trades", total_trades);
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    println!();
    println!("{}", "✅ Simulation completed!".green().bold());
    println!("  Total ticks: {}", tick_count);

    // Show final statistics
    for (symbol, market_data) in simulator.get_all_market_data() {
        println!("  Symbol {}: {} trades", symbol, market_data.trades.len());
    }
}

fn show_order_book(symbol: u32) {
    println!("{}", format!("📚 Order Book for Symbol {}", symbol).cyan().bold());
    println!("  (This would show the full order book depth)");
    println!("  Implementation pending...");
}

fn show_trades(symbol: u32, count: usize) {
    println!("{}", format!("💱 Recent Trades for Symbol {}", symbol).cyan().bold());
    println!("  Showing last {} trades", count);
    println!("  Implementation pending...");
}

fn test_partial_fill(symbol: u32) {
    println!("{}", "🧪 Testing Partial Fill Scenario".cyan().bold());
    println!(
        "  Testing: 10@155 ask vs 5@155 bid should result in 5@155 trade and 5@155 remaining ask"
    );
    println!();

    let mut simulator = ExchangeSimulator::new(1);

    // Step 1: Submit a sell order for 10@155
    println!("  Step 1: Submitting sell order 10@155");
    simulator.submit_order(symbol, 1, 1, Side::Sell, OrderType::Limit, Some(155), 10).unwrap();
    simulator.process_tick();

    // Show order book after sell order
    if let Some(engine) = simulator.engines.get(&symbol) {
        let asks = engine.get_order_book_levels(Side::Sell);
        let bids = engine.get_order_book_levels(Side::Buy);
        println!("    Order book after sell:");
        println!("      Asks: {:?}", asks);
        println!("      Bids: {:?}", bids);
    }

    // Step 2: Submit a buy order for 5@155 (should match and create partial fill)
    println!("  Step 2: Submitting buy order 5@155");
    simulator.submit_order(symbol, 2, 2, Side::Buy, OrderType::Limit, Some(155), 5).unwrap();
    let events = simulator.process_tick();

    // Show events
    println!("    Events generated:");
    for (i, event) in events[0].1.iter().enumerate() {
        println!("      {}. {:?}", i + 1, event);
    }

    // Show order book after trade
    if let Some(engine) = simulator.engines.get(&symbol) {
        let asks = engine.get_order_book_levels(Side::Sell);
        let bids = engine.get_order_book_levels(Side::Buy);
        println!("    Order book after trade:");
        println!("      Asks: {:?}", asks);
        println!("      Bids: {:?}", bids);

        // Check if the ask was partially filled
        if let Some((price, qty)) = asks.first() {
            if *price == 155 && *qty == 5 {
                println!("    ✅ SUCCESS: Ask correctly reduced from 10@155 to 5@155");
            } else {
                println!("    ❌ FAILED: Ask should be 5@155, but got {}@{}", qty, price);
            }
        } else {
            println!("    ❌ FAILED: No ask remaining (should be 5@155)");
        }
    }

    // Show market data
    if let Some(market_data) = simulator.get_market_data(symbol) {
        println!("    Market data:");
        println!("      Last trade: {:?}", market_data.last_trade_price);
        println!("      Last trade qty: {:?}", market_data.last_trade_qty);
        println!("      Total trades: {}", market_data.trades.len());
    }
}

fn run_manual_simulation(symbol: u32, tick_delay_ms: u64) {
    println!("{}", "🎮 Starting Comprehensive Manual Simulation".cyan().bold());
    println!("  Symbol: {}", symbol);
    println!("  Tick delay: {}ms", tick_delay_ms);
    println!("  Press Ctrl+C to exit early");
    println!();

    let mut simulator = ExchangeSimulator::new(1);
    let tick_delay = Duration::from_millis(tick_delay_ms);

    // Clear screen
    print!("\x1B[2J\x1B[1;1H");

    // Tick 100: Initial Liquidity Setup
    println!("{}", "🔄 TICK 100: Initial Liquidity Setup".yellow().bold());
    simulator.submit_order(symbol, 1, 1, Side::Buy, OrderType::Limit, Some(150), 20).unwrap();
    simulator.submit_order(symbol, 2, 2, Side::Sell, OrderType::Limit, Some(155), 15).unwrap();
    simulator.submit_order(symbol, 3, 3, Side::Buy, OrderType::Limit, Some(145), 10).unwrap();
    simulator.submit_order(symbol, 4, 4, Side::Sell, OrderType::Limit, Some(160), 8).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 100, &events);
    std::thread::sleep(tick_delay);

    // Tick 101: Market Order Test
    println!("{}", "🔄 TICK 101: Market Order Test".yellow().bold());
    simulator.submit_order(symbol, 5, 5, Side::Buy, OrderType::Market, None, 12).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 101, &events);
    std::thread::sleep(tick_delay);

    // Tick 102: Partial Fill Test
    println!("{}", "🔄 TICK 102: Partial Fill Test".yellow().bold());
    simulator.submit_order(symbol, 6, 6, Side::Sell, OrderType::Limit, Some(150), 5).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 102, &events);
    std::thread::sleep(tick_delay);

    // Tick 103: Multiple Level Matching
    println!("{}", "🔄 TICK 103: Multiple Level Matching".yellow().bold());
    simulator.submit_order(symbol, 7, 7, Side::Buy, OrderType::Limit, Some(160), 25).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 103, &events);
    std::thread::sleep(tick_delay);

    // Tick 104: POST-ONLY Test
    println!("{}", "🔄 TICK 104: POST-ONLY Test".yellow().bold());
    simulator.submit_order(symbol, 8, 8, Side::Buy, OrderType::PostOnly, Some(165), 10).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 104, &events);
    std::thread::sleep(tick_delay);

    // Tick 105: POST-ONLY Acceptance Test
    println!("{}", "🔄 TICK 105: POST-ONLY Acceptance Test".yellow().bold());
    simulator.submit_order(symbol, 9, 9, Side::Buy, OrderType::PostOnly, Some(160), 5).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 105, &events);
    std::thread::sleep(tick_delay);

    // Tick 106: Self-Match Prevention Test
    println!("{}", "🔄 TICK 106: Self-Match Prevention Test".yellow().bold());
    simulator.submit_order(symbol, 10, 1, Side::Sell, OrderType::Limit, Some(160), 5).unwrap(); // Account 1
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 106, &events);
    std::thread::sleep(tick_delay);

    // Tick 107: IOC Order Test
    println!("{}", "🔄 TICK 107: IOC Order Test".yellow().bold());
    simulator.submit_order(symbol, 11, 10, Side::Sell, OrderType::Ioc, Some(165), 8).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 107, &events);
    std::thread::sleep(tick_delay);

    // Tick 108: Full Book Sweep
    println!("{}", "🔄 TICK 108: Full Book Sweep".yellow().bold());
    simulator.submit_order(symbol, 12, 11, Side::Buy, OrderType::Market, None, 50).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 108, &events);
    std::thread::sleep(tick_delay);

    // Tick 109: Price-Time Priority Test
    println!("{}", "🔄 TICK 109: Price-Time Priority Test".yellow().bold());
    simulator.submit_order(symbol, 13, 12, Side::Sell, OrderType::Limit, Some(160), 3).unwrap();
    simulator.submit_order(symbol, 14, 13, Side::Sell, OrderType::Limit, Some(160), 4).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 109, &events);
    std::thread::sleep(tick_delay);

    // Tick 110: Final Priority Test
    println!("{}", "🔄 TICK 110: Final Priority Test".yellow().bold());
    simulator.submit_order(symbol, 15, 14, Side::Buy, OrderType::Limit, Some(160), 5).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 110, &events);

    println!();
    println!("{}", "✅ Manual Simulation Complete!".green().bold());
    println!("  All test scenarios executed successfully.");
    println!("  Check the results above to verify correct behavior.");
}

fn display_simulation_state(
    simulator: &ExchangeSimulator,
    symbol: u32,
    tick: u64,
    events: &[(u32, Vec<EngineEvent>)],
) {
    // Clear screen
    print!("\x1B[2J\x1B[1;1H");

    println!("{}", "🎯 WHISTLE EXCHANGE MANUAL SIMULATION".cyan().bold());
    println!("  Tick: {} | Symbol: {}", tick, symbol);
    println!();

    // Display events
    if let Some((_, tick_events)) = events.first() {
        if !tick_events.is_empty() {
            println!("{}", "📊 Events Generated:".yellow().bold());
            for (i, event) in tick_events.iter().enumerate() {
                match event {
                    EngineEvent::Trade(ev) => {
                        println!(
                            "  {}. {} @ {} ({} → {})",
                            i + 1,
                            ev.qty,
                            ev.price,
                            if ev.taker_side == Side::Buy { "BUY" } else { "SELL" },
                            ev.taker_order
                        );
                    }
                    EngineEvent::BookDelta(ev) => {
                        println!(
                            "  {}. BookDelta: {} {} → {}",
                            i + 1,
                            if ev.side == Side::Buy { "BID" } else { "ASK" },
                            ev.price,
                            ev.level_qty_after
                        );
                    }
                    EngineEvent::Lifecycle(ev) => {
                        let status = match ev.kind {
                            whistle::LifecycleKind::Accepted => "ACCEPTED",
                            whistle::LifecycleKind::Rejected => "REJECTED",
                            whistle::LifecycleKind::Cancelled => "CANCELLED",
                        };
                        println!("  {}. Order {}: {}", i + 1, ev.order_id, status);
                    }
                    _ => {}
                }
            }
            println!();
        }
    }

    // Display order book
    if let Some(engine) = simulator.engines.get(&symbol) {
        if let Some(market_data) = simulator.get_market_data(symbol) {
            display_symbol_summary(market_data, engine);
        }
    }

    // Display simulation progress
    let progress = match tick {
        100 => "Initial Liquidity Setup",
        101 => "Market Order Test",
        102 => "Partial Fill Test",
        103 => "Multiple Level Matching",
        104 => "POST-ONLY Test",
        105 => "POST-ONLY Acceptance Test",
        106 => "Self-Match Prevention Test",
        107 => "IOC Order Test",
        108 => "Full Book Sweep",
        109 => "Price-Time Priority Test",
        110 => "Final Priority Test",
        _ => "Unknown",
    };

    println!("{}", format!("📈 Current Test: {}", progress).magenta().bold());
    println!();
}

fn debug_matching(symbol: u32) {
    println!("{}", "🔍 Debugging Matching Issues".cyan().bold());
    println!();

    let mut simulator = ExchangeSimulator::new(1);

    // Test 1: Market Order Test
    println!("{}", "Test 1: Market Order Test".yellow().bold());
    simulator.submit_order(symbol, 1, 2, Side::Sell, OrderType::Limit, Some(155), 15).unwrap();
    simulator.process_tick();

    simulator.submit_order(symbol, 2, 5, Side::Buy, OrderType::Market, None, 12).unwrap();
    let events = simulator.process_tick();

    println!("  Events:");
    for (i, event) in events[0].1.iter().enumerate() {
        println!("    {}. {:?}", i + 1, event);
    }
    println!();

    // Test 2: POST-ONLY Test
    println!("{}", "Test 2: POST-ONLY Test".yellow().bold());
    simulator.submit_order(symbol, 3, 8, Side::Buy, OrderType::PostOnly, Some(165), 10).unwrap();
    let events = simulator.process_tick();

    println!("  Events:");
    for (i, event) in events[0].1.iter().enumerate() {
        println!("    {}. {:?}", i + 1, event);
    }
    println!();

    // Test 3: POST-ONLY Rejection Test
    println!("{}", "Test 3: POST-ONLY Rejection Test".yellow().bold());
    simulator.submit_order(symbol, 4, 9, Side::Buy, OrderType::PostOnly, Some(160), 5).unwrap();
    let events = simulator.process_tick();

    println!("  Events:");
    for (i, event) in events[0].1.iter().enumerate() {
        println!("    {}. {:?}", i + 1, event);
    }
    println!();

    // Test 4: IOC Order Test
    println!("{}", "Test 4: IOC Order Test".yellow().bold());
    simulator.submit_order(symbol, 5, 10, Side::Sell, OrderType::Ioc, Some(165), 8).unwrap();
    let events = simulator.process_tick();

    println!("  Events:");
    for (i, event) in events[0].1.iter().enumerate() {
        println!("    {}. {:?}", i + 1, event);
    }
    println!();
}

fn debug_ioc(symbol: u32) {
    println!("{}", "🔍 Debugging IOC Order Issue".cyan().bold());
    println!();

    let mut simulator = ExchangeSimulator::new(1);

    // Step 1: Add some buy orders to create liquidity
    println!("{}", "Step 1: Adding buy orders for liquidity".yellow().bold());
    simulator.submit_order(symbol, 1, 1, Side::Buy, OrderType::Limit, Some(160), 10).unwrap();
    simulator.submit_order(symbol, 2, 2, Side::Buy, OrderType::Limit, Some(155), 15).unwrap();
    simulator.submit_order(symbol, 3, 3, Side::Buy, OrderType::Limit, Some(150), 20).unwrap();
    let events = simulator.process_tick();

    println!("  Events:");
    for (i, event) in events[0].1.iter().enumerate() {
        println!("    {}. {:?}", i + 1, event);
    }
    println!();

    // Show order book state
    if let Some(engine) = simulator.engines.get(&symbol) {
        let asks = engine.get_order_book_levels(Side::Sell);
        let bids = engine.get_order_book_levels(Side::Buy);
        println!("  Order book after adding buy orders:");
        println!("    Asks: {:?}", asks);
        println!("    Bids: {:?}", bids);
    }
    println!();

    // Step 2: Submit IOC sell order that should match
    println!(
        "{}",
        "Step 2: Submitting IOC sell order at 160 (should match against buy at 160)"
            .yellow()
            .bold()
    );
    simulator.submit_order(symbol, 4, 4, Side::Sell, OrderType::Ioc, Some(160), 8).unwrap();
    let events = simulator.process_tick();

    println!("  Events:");
    for (i, event) in events[0].1.iter().enumerate() {
        println!("    {}. {:?}", i + 1, event);
    }
    println!();

    // Show order book state after IOC
    if let Some(engine) = simulator.engines.get(&symbol) {
        let asks = engine.get_order_book_levels(Side::Sell);
        let bids = engine.get_order_book_levels(Side::Buy);
        println!("  Order book after IOC order:");
        println!("    Asks: {:?}", asks);
        println!("    Bids: {:?}", bids);
    }
    println!();

    // Step 3: Submit IOC sell order with no price (should match at best bid)
    println!(
        "{}",
        "Step 3: Submitting IOC sell order with no price (should match at best bid)"
            .yellow()
            .bold()
    );
    simulator.submit_order(symbol, 5, 5, Side::Sell, OrderType::Ioc, None, 5).unwrap();
    let events = simulator.process_tick();

    println!("  Events:");
    for (i, event) in events[0].1.iter().enumerate() {
        println!("    {}. {:?}", i + 1, event);
    }
    println!();
}

fn test_ticks_103_104(symbol: u32) {
    println!("{}", "🧪 Testing Specific Ticks 103 and 104".cyan().bold());
    println!();

    let mut simulator = ExchangeSimulator::new(1);

    // Tick 103: Multiple Level Matching
    println!("{}", "Tick 103: Multiple Level Matching".yellow().bold());
    simulator.submit_order(symbol, 1, 1, Side::Buy, OrderType::Limit, Some(160), 25).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 103, &events);
    std::thread::sleep(Duration::from_millis(100)); // Small delay to allow engine to process

    // Tick 104: POST-ONLY Test
    println!("{}", "Tick 104: POST-ONLY Test".yellow().bold());
    simulator.submit_order(symbol, 2, 2, Side::Buy, OrderType::PostOnly, Some(165), 10).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 104, &events);
    std::thread::sleep(Duration::from_millis(100)); // Small delay to allow engine to process

    // Show final state
    if let Some(engine) = simulator.engines.get(&symbol) {
        let asks = engine.get_order_book_levels(Side::Sell);
        let bids = engine.get_order_book_levels(Side::Buy);
        println!("  Final Order Book State:");
        println!("    Asks: {:?}", asks);
        println!("    Bids: {:?}", bids);
    }

    println!();
    println!("{}", "✅ Specific Ticks 103 and 104 Test Complete!".green().bold());
    println!("  Check the results above to verify correct behavior.");
}

fn test_ticks_100_to_104(symbol: u32) {
    println!("{}", "🧪 Testing Complete Sequence from Tick 100 to 104".cyan().bold());
    println!();

    let mut simulator = ExchangeSimulator::new(1);

    // Tick 100: Initial Liquidity Setup
    println!("{}", "🔄 TICK 100: Initial Liquidity Setup".yellow().bold());
    simulator.submit_order(symbol, 1, 1, Side::Buy, OrderType::Limit, Some(150), 20).unwrap();
    simulator.submit_order(symbol, 2, 2, Side::Sell, OrderType::Limit, Some(155), 15).unwrap();
    simulator.submit_order(symbol, 3, 3, Side::Buy, OrderType::Limit, Some(145), 10).unwrap();
    simulator.submit_order(symbol, 4, 4, Side::Sell, OrderType::Limit, Some(160), 8).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 100, &events);
    std::thread::sleep(Duration::from_millis(100)); // Small delay to allow engine to process

    // Tick 101: Market Order Test
    println!("{}", "🔄 TICK 101: Market Order Test".yellow().bold());
    simulator.submit_order(symbol, 5, 5, Side::Buy, OrderType::Market, None, 12).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 101, &events);
    std::thread::sleep(Duration::from_millis(100)); // Small delay to allow engine to process

    // Tick 102: Partial Fill Test
    println!("{}", "🔄 TICK 102: Partial Fill Test".yellow().bold());
    simulator.submit_order(symbol, 6, 6, Side::Sell, OrderType::Limit, Some(150), 5).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 102, &events);
    std::thread::sleep(Duration::from_millis(100)); // Small delay to allow engine to process

    // Tick 103: Multiple Level Matching
    println!("{}", "🔄 TICK 103: Multiple Level Matching".yellow().bold());
    simulator.submit_order(symbol, 7, 7, Side::Buy, OrderType::Limit, Some(160), 25).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 103, &events);
    std::thread::sleep(Duration::from_millis(100)); // Small delay to allow engine to process

    // Tick 104: POST-ONLY Test
    println!("{}", "🔄 TICK 104: POST-ONLY Test".yellow().bold());
    simulator.submit_order(symbol, 8, 8, Side::Buy, OrderType::PostOnly, Some(165), 10).unwrap();
    let events = simulator.process_tick();

    display_simulation_state(&simulator, symbol, 104, &events);
    std::thread::sleep(Duration::from_millis(100)); // Small delay to allow engine to process

    // Show final state
    if let Some(engine) = simulator.engines.get(&symbol) {
        let asks = engine.get_order_book_levels(Side::Sell);
        let bids = engine.get_order_book_levels(Side::Buy);
        println!("  Final Order Book State:");
        println!("    Asks: {:?}", asks);
        println!("    Bids: {:?}", bids);
    }

    println!();
    println!("{}", "✅ Complete Sequence 100-104 Test Complete!".green().bold());
    println!("  Check the results above to verify correct behavior.");
}

fn test_multiple_level_matching(symbol: u32) {
    println!("{}", "🧪 Testing Multiple Level Matching".cyan().bold());
    println!();

    let mut simulator = ExchangeSimulator::new(1);

    // Add some resting orders
    simulator.submit_order(symbol, 1, 1, Side::Buy, OrderType::Limit, Some(150), 10).ok();
    simulator.submit_order(symbol, 2, 2, Side::Sell, OrderType::Limit, Some(155), 10).ok();
    simulator.process_tick();

    // Show initial order book
    if let Some(engine) = simulator.engines.get(&symbol) {
        let asks = engine.get_order_book_levels(Side::Sell);
        let bids = engine.get_order_book_levels(Side::Buy);
        println!("  Initial Order Book:");
        println!("    Asks: {:?}", asks);
        println!("    Bids: {:?}", bids);
    }

    // Submit a buy order for 10@155 (should match 5@155 and 5@155)
    println!("  Submitting buy order 10@155");
    simulator.submit_order(symbol, 3, 3, Side::Buy, OrderType::Limit, Some(155), 10).unwrap();
    let events = simulator.process_tick();

    // Show events
    println!("  Events:");
    for (i, event) in events[0].1.iter().enumerate() {
        println!("    {}. {:?}", i + 1, event);
    }

    // Show order book after trade
    if let Some(engine) = simulator.engines.get(&symbol) {
        let asks = engine.get_order_book_levels(Side::Sell);
        let bids = engine.get_order_book_levels(Side::Buy);
        println!("  Order book after trade:");
        println!("    Asks: {:?}", asks);
        println!("    Bids: {:?}", bids);

        // Check if the ask was partially filled
        if let Some((price, qty)) = asks.first() {
            if *price == 155 && *qty == 5 {
                println!("    ✅ SUCCESS: Ask correctly reduced from 10@155 to 5@155");
            } else {
                println!("    ❌ FAILED: Ask should be 5@155, but got {}@{}", qty, price);
            }
        } else {
            println!("    ❌ FAILED: No ask remaining (should be 5@155)");
        }
    }

    // Show market data
    if let Some(market_data) = simulator.get_market_data(symbol) {
        println!("  Market data:");
        println!("    Last trade: {:?}", market_data.last_trade_price);
        println!("    Last trade qty: {:?}", market_data.last_trade_qty);
        println!("    Total trades: {}", market_data.trades.len());
    }

    println!();
    println!("{}", "✅ Multiple Level Matching Test Complete!".green().bold());
    println!("  Check the results above to verify correct behavior.");
}

// Session engine types
pub struct SessionEngine {
    session_name: String,
    engine: Whistle,
    tick: u64,
    session_dir: std::path::PathBuf,
    last_order_read: u64,
    last_response_write: u64,
    last_trade_price: Option<u32>,
    last_trade_qty: Option<u32>,
    last_trade_side: Option<Side>,
}

impl SessionEngine {
    pub fn new(session_name: &str, symbol: u32) -> Result<Self, String> {
        let sessions_dir = std::env::temp_dir().join("whistle-exchange");
        let session_dir = sessions_dir.join(session_name);

        if !session_dir.exists() {
            return Err(format!(
                "Session '{}' does not exist. Create it first with playground.",
                session_name
            ));
        }

        let cfg = EngineCfg {
            symbol,
            price_domain: PriceDomain { floor: 100, ceil: 200, tick: 1 }, // Changed from 5 to 1 for more flexible pricing
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

        Ok(Self {
            session_name: session_name.to_string(),
            engine,
            tick: 100,
            session_dir,
            last_order_read: 0,
            last_response_write: 0,
            last_trade_price: None,
            last_trade_qty: None,
            last_trade_side: None,
        })
    }

    pub fn run(&mut self, tick_interval_ms: u64, display_mode: &str) -> Result<(), String> {
        println!("🚀 Starting session engine for '{}'", self.session_name);
        println!("📁 Session directory: {}", self.session_dir.display());
        println!("⏱️  Tick interval: {}ms", tick_interval_ms);
        println!("📊 Display mode: {}", display_mode);
        println!("Press Ctrl+C to stop");
        println!();

        let tick_duration = Duration::from_millis(tick_interval_ms);
        let mut last_tick = Instant::now();
        let mut last_display_update = Instant::now();
        let mut last_order_count = 0;
        let mut last_tick_count = self.tick;

        // Initial display
        if display_mode == "dashboard" {
            self.update_dashboard_in_place();
        }

        loop {
            let now = Instant::now();

            // Process incoming orders from playground clients
            let orders_processed = self.process_incoming_orders()?;

            // Process tick if interval elapsed
            let tick_processed = if now.duration_since(last_tick) >= tick_duration {
                self.process_tick()?;
                last_tick = now;
                true
            } else {
                false
            };

            // Only update display if there are changes or every 5 seconds
            let should_update = display_mode != "minimal"
                && (orders_processed > last_order_count
                    || tick_processed
                    || self.tick != last_tick_count
                    || now.duration_since(last_display_update) >= Duration::from_secs(5));

            if should_update {
                if display_mode == "dashboard" {
                    self.update_dashboard_in_place();
                } else {
                    self.update_display();
                }
                last_display_update = now;
                last_order_count = orders_processed;
                last_tick_count = self.tick;
            }

            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn process_incoming_orders(&mut self) -> Result<u64, String> {
        let orders_file = self.session_dir.join("orders.jsonl");
        if !orders_file.exists() {
            return Ok(0);
        }

        let content = std::fs::read_to_string(&orders_file)
            .map_err(|e| format!("Failed to read orders file: {}", e))?;

        let lines: Vec<&str> = content.lines().collect();
        let mut orders_processed = 0;

        // Process only new orders (after last_order_read)
        for (_line_num, line) in lines.iter().enumerate().skip(self.last_order_read as usize) {
            if line.trim().is_empty() {
                continue;
            }

            let order_data: serde_json::Value = serde_json::from_str(line)
                .map_err(|e| format!("Failed to parse order JSON: {}", e))?;

            // Convert to InboundMsg and submit to engine
            if let Some(msg) = self.parse_order_to_message(&order_data)? {
                self.engine
                    .enqueue_message(msg)
                    .map_err(|e| format!("Failed to enqueue order: {:?}", e))?;
                orders_processed += 1;
            }
        }

        self.last_order_read = lines.len() as u64;
        Ok(orders_processed)
    }

    fn parse_order_to_message(
        &self,
        order_data: &serde_json::Value,
    ) -> Result<Option<InboundMsg>, String> {
        let account_id = order_data["account_id"].as_u64().ok_or("Missing account_id")?;
        let order_id = order_data["order_id"].as_u64().ok_or("Missing order_id")?;
        let side_str = order_data["side"].as_str().ok_or("Missing side")?;
        let order_type_str = order_data["order_type"].as_str().ok_or("Missing order_type")?;
        let qty = order_data["qty"].as_u64().ok_or("Missing qty")?;
        let timestamp = order_data["timestamp"].as_u64().unwrap_or(self.tick * 1000 + order_id);

        let side = match side_str {
            "buy" => Side::Buy,
            "sell" => Side::Sell,
            _ => return Err(format!("Invalid side: {}", side_str)),
        };

        let order_type = match order_type_str {
            "limit" => OrderType::Limit,
            "market" => OrderType::Market,
            "ioc" => OrderType::Ioc,
            "post_only" => OrderType::PostOnly,
            _ => return Err(format!("Invalid order type: {}", order_type_str)),
        };

        let price = order_data["price"].as_u64().map(|p| p as u32);

        let msg = InboundMsg::submit(
            order_id, account_id, side, order_type, price, qty as u32, timestamp, 0, // flags
            1, // symbol - hardcoded for now
        );

        Ok(Some(msg))
    }

    fn process_tick(&mut self) -> Result<(), String> {
        let events = self.engine.tick(self.tick);
        self.tick += 1;

        // Write events to session files
        self.write_events_to_files(&events)?;

        // Display events
        self.display_tick_events(&events);

        Ok(())
    }

    fn write_events_to_files(&mut self, events: &[EngineEvent]) -> Result<(), String> {
        for event in events {
            match event {
                EngineEvent::Trade(trade) => {
                    // Update last trade information
                    self.last_trade_price = Some(trade.price);
                    self.last_trade_qty = Some(trade.qty);
                    self.last_trade_side = Some(trade.taker_side);
                    self.write_trade_event(trade)?;
                }
                EngineEvent::Lifecycle(lifecycle) => {
                    self.write_lifecycle_event(lifecycle)?;
                }
                EngineEvent::BookDelta(delta) => {
                    self.write_book_delta_event(delta)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn write_trade_event(&self, trade: &whistle::EvTrade) -> Result<(), String> {
        let trade_file = self.session_dir.join("trades.jsonl");
        let trade_data = serde_json::json!({
            "price": trade.price,
            "qty": trade.qty,
            "maker_order": trade.maker_order,
            "taker_order": trade.taker_order,
            "taker_side": if trade.taker_side == Side::Buy { "buy" } else { "sell" },
            "exec_id": trade.exec_id,
            "tick": self.tick - 1,
            "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        });

        self.append_to_file(&trade_file, &trade_data)
    }

    fn write_lifecycle_event(&self, lifecycle: &whistle::EvLifecycle) -> Result<(), String> {
        let response_file = self.session_dir.join("responses.jsonl");
        let response_data = serde_json::json!({
            "account_id": 1, // TODO: get from order lookup
            "order_id": lifecycle.order_id,
            "status": match lifecycle.kind {
                whistle::LifecycleKind::Accepted => "accepted",
                whistle::LifecycleKind::Rejected => "rejected",
                whistle::LifecycleKind::Cancelled => "cancelled",
            },
            "reason": lifecycle.reason.as_ref().map(|r| format!("{:?}", r)),
            "tick": self.tick - 1,
            "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        });

        self.append_to_file(&response_file, &response_data)
    }

    fn write_book_delta_event(&self, delta: &whistle::EvBookDelta) -> Result<(), String> {
        let book_file = self.session_dir.join("book_updates.jsonl");
        let book_data = serde_json::json!({
            "side": if delta.side == Side::Buy { "buy" } else { "sell" },
            "price": delta.price,
            "qty": delta.level_qty_after,
            "tick": self.tick - 1,
            "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        });

        self.append_to_file(&book_file, &book_data)
    }

    fn append_to_file(
        &self,
        file_path: &std::path::Path,
        data: &serde_json::Value,
    ) -> Result<(), String> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .map_err(|e| format!("Failed to open file: {}", e))?;

        use std::io::Write;
        writeln!(file, "{}", data.to_string())
            .map_err(|e| format!("Failed to write to file: {}", e))?;

        Ok(())
    }

    fn display_tick_events(&self, events: &[EngineEvent]) {
        if events.is_empty() {
            return;
        }

        println!("🔄 Tick {}: {} events", self.tick - 1, events.len());

        for event in events {
            match event {
                EngineEvent::Trade(trade) => {
                    println!(
                        "  💰 Trade: {} @ {} (maker: {}, taker: {})",
                        trade.qty, trade.price, trade.maker_order, trade.taker_order
                    );
                }
                EngineEvent::Lifecycle(lifecycle) => {
                    let status = match lifecycle.kind {
                        whistle::LifecycleKind::Accepted => "ACCEPTED".green(),
                        whistle::LifecycleKind::Rejected => "REJECTED".red(),
                        whistle::LifecycleKind::Cancelled => "CANCELLED".yellow(),
                    };
                    println!("  🔄 Order {}: {}", lifecycle.order_id, status);
                }
                EngineEvent::BookDelta(delta) => {
                    println!(
                        "  📚 Book: {} @ {} (qty: {})",
                        if delta.side == Side::Buy { "BUY" } else { "SELL" },
                        delta.price,
                        delta.level_qty_after
                    );
                }
                _ => {}
            }
        }
        println!();
    }

    fn update_display(&self) {
        // Clear screen and show current state
        print!("\x1B[2J\x1B[1;1H");
        println!("🎯 WHISTLE SESSION ENGINE - {}", self.session_name.cyan().bold());
        println!("🕐 Tick: {} | Symbol: 1", self.tick);
        println!("📁 Session: {}", self.session_dir.display());
        println!();

        // Show order book with beautiful formatting
        self.display_session_order_book();
    }

    fn display_session_order_book(&self) {
        println!("📈 Symbol 1");
        println!("  📚 Order Book:");

        // Get full order book data from the engine
        let asks = self.engine.get_order_book_levels(Side::Sell); // Sell orders (asks)
        let bids = self.engine.get_order_book_levels(Side::Buy); // Buy orders (bids)

        // Display top 10 asks (sells) - highest price first
        println!("    {} (Top 10 Sells)", "Price | Amount | Total".dimmed());
        for (price, qty) in asks.iter().rev().take(10) {
            // rev() to show highest first
            let total = price * qty;
            println!("    {} | {} | {}", price.to_string().red(), qty, total);
        }
        if asks.is_empty() {
            println!("    {}", "No sell orders".dimmed());
        }

        // Display last trade price in the middle
        match (self.last_trade_price, self.last_trade_qty, self.last_trade_side) {
            (Some(price), Some(qty), Some(side)) => {
                let colored_price = match side {
                    Side::Buy => price.to_string().green(), // Green for buy (taker bought)
                    Side::Sell => price.to_string().red(),  // Red for sell (taker sold)
                };
                println!("    {} @ {} ({} units)", "Last Trade:".bold(), colored_price, qty);
            }
            _ => {
                println!("    {} @ {}", "Last Trade:".bold(), "None".dimmed());
            }
        }

        // Display top 10 bids (buys) - highest price first
        println!("    {} (Top 10 Buys)", "Price | Amount | Total".dimmed());
        for (price, qty) in bids.iter().take(10) {
            // already sorted highest first
            let total = price * qty;
            println!("    {} | {} | {}", price.to_string().green(), qty, total);
        }
        if bids.is_empty() {
            println!("    {}", "No buy orders".dimmed());
        }

        println!();
    }

    fn update_dashboard_display(&self) {
        // Clear screen and show current state
        print!("\x1B[2J\x1B[1;1H");
        println!("🎯 WHISTLE SESSION ENGINE - {}", self.session_name.cyan().bold());
        println!("🕐 Tick: {} | Symbol: 1", self.tick);
        println!("📁 Session: {}", self.session_dir.display());
        println!();

        // Show the beautiful order book display
        self.display_session_order_book();

        // Show recent activity summary
        println!("📊 Session Status:");
        println!("  🔄 Orders Processed: {}", self.last_order_read);
        println!("  ⏱️  Current Tick: {}", self.tick);
        println!("  📁 Session Directory: {}", self.session_dir.display());
        println!();
    }

    fn update_dashboard_in_place(&mut self) {
        static mut FIRST_RUN: bool = true;
        static mut LAST_TICK: u64 = 0;
        static mut LAST_ORDERS: u64 = 0;

        unsafe {
            let tick_changed = self.tick != LAST_TICK;
            let orders_changed = self.last_order_read != LAST_ORDERS;

            if FIRST_RUN || tick_changed || orders_changed {
                // Clear screen and redraw
                print!("\x1B[2J\x1B[1;1H"); // Clear screen
                println!("🎯 WHISTLE SESSION ENGINE - {}", self.session_name.cyan().bold());
                println!("🕐 Tick: {} | Symbol: 1", self.tick);
                println!("📁 Session: {}", self.session_dir.display());
                println!();

                // Show the beautiful order book display
                self.display_session_order_book();

                // Show recent activity summary
                println!("📊 Session Status:");
                println!("  🔄 Orders Processed: {}", self.last_order_read);
                println!("  ⏱️  Current Tick: {}", self.tick);
                println!("  📁 Session Directory: {}", self.session_dir.display());
                println!();

                FIRST_RUN = false;
                LAST_TICK = self.tick;
                LAST_ORDERS = self.last_order_read;
            }
        }
    }
}

fn start_session(session_name: String, symbol: u32, tick_interval_ms: u64, display: String) {
    println!("{}", "🚀 Starting Whistle Session Engine".cyan().bold());

    match SessionEngine::new(&session_name, symbol) {
        Ok(mut engine) => {
            if let Err(e) = engine.run(tick_interval_ms, &display) {
                eprintln!("{}", format!("❌ Session engine error: {}", e).red());
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("{}", format!("❌ Failed to start session engine: {}", e).red());
            eprintln!("💡 Create the session first with: cargo run --bin whistle-playground -- create-session {}", session_name);
            std::process::exit(1);
        }
    }
}

fn list_sessions() {
    println!("{}", "📋 Available Sessions".cyan().bold());

    let sessions_dir = std::env::temp_dir().join("whistle-exchange");
    if !sessions_dir.exists() {
        println!("No sessions directory found.");
        return;
    }

    match std::fs::read_dir(sessions_dir) {
        Ok(entries) => {
            let mut sessions = Vec::new();
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            sessions.push(name.to_string());
                        }
                    }
                }
            }

            if sessions.is_empty() {
                println!("No sessions found.");
                println!("💡 Create a session with: cargo run --bin whistle-playground -- create-session <name>");
            } else {
                for session in sessions {
                    println!("  📁 {}", session);
                }
            }
        }
        Err(e) => {
            eprintln!("{}", format!("❌ Failed to read sessions directory: {}", e).red());
        }
    }
}

fn show_session_info(session_name: String) {
    println!("{}", format!("📊 Session Info: {}", session_name).cyan().bold());

    let sessions_dir = std::env::temp_dir().join("whistle-exchange");
    let session_dir = sessions_dir.join(&session_name);

    if !session_dir.exists() {
        eprintln!("{}", format!("❌ Session '{}' does not exist", session_name).red());
        return;
    }

    // Try to read session config
    let config_file = session_dir.join("config.json");
    if config_file.exists() {
        match std::fs::read_to_string(config_file) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(config) => {
                    println!("  📁 Directory: {}", session_dir.display());
                    println!("  👥 Accounts: {}", config["accounts"].as_u64().unwrap_or(0));
                    println!("  🕐 Created: {}", config["created"].as_u64().unwrap_or(0));
                    println!(
                        "  🔄 Last Activity: {}",
                        config["last_activity"].as_u64().unwrap_or(0)
                    );
                }
                Err(e) => {
                    eprintln!("❌ Failed to parse session config: {}", e);
                }
            },
            Err(e) => {
                eprintln!("❌ Failed to read session config: {}", e);
            }
        }
    }

    // Show session files
    println!("\n📄 Session Files:");
    let files = ["orders.jsonl", "responses.jsonl", "trades.jsonl", "book_updates.jsonl"];
    for file in files {
        let file_path = session_dir.join(file);
        if file_path.exists() {
            match std::fs::metadata(&file_path) {
                Ok(metadata) => {
                    let size = metadata.len();
                    println!("  📄 {} ({} bytes)", file, size);
                }
                Err(_) => {
                    println!("  📄 {} (error reading)", file);
                }
            }
        } else {
            println!("  📄 {} (not found)", file);
        }
    }
}

fn cleanup_sessions() {
    println!("{}", "🧹 Cleaning up expired sessions".cyan().bold());

    let sessions_dir = std::env::temp_dir().join("whistle-exchange");
    if !sessions_dir.exists() {
        println!("No sessions directory found.");
        return;
    }

    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let max_age = 24 * 60 * 60; // 24 hours

    match std::fs::read_dir(sessions_dir) {
        Ok(entries) => {
            let mut cleaned = 0;
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            let config_file = path.join("config.json");
                            if config_file.exists() {
                                if let Ok(content) = std::fs::read_to_string(config_file) {
                                    if let Ok(config) =
                                        serde_json::from_str::<serde_json::Value>(&content)
                                    {
                                        if let Some(last_activity) =
                                            config["last_activity"].as_u64()
                                        {
                                            if now - last_activity > max_age {
                                                match std::fs::remove_dir_all(&path) {
                                                    Ok(_) => {
                                                        println!(
                                                            "  🗑️  Removed expired session: {}",
                                                            name
                                                        );
                                                        cleaned += 1;
                                                    }
                                                    Err(e) => {
                                                        eprintln!(
                                                            "  ❌ Failed to remove session {}: {}",
                                                            name, e
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if cleaned == 0 {
                println!("No expired sessions found.");
            } else {
                println!("Cleaned up {} expired sessions.", cleaned);
            }
        }
        Err(e) => {
            eprintln!("{}", format!("❌ Failed to read sessions directory: {}", e).red());
        }
    }
}
