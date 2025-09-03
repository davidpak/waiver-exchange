use crate::session::{SessionManager, SessionEngine};
use colored::*;
use std::io::{self, Write};

use std::env;

pub struct InteractiveCLI {
    session_manager: SessionManager,
    current_session: Option<String>,
    current_account: Option<u32>,
    session_engine: Option<SessionEngine>,
}

impl InteractiveCLI {
    pub fn new() -> Self {
        Self {
            session_manager: SessionManager::new(),
            current_session: None,
            current_account: None,
            session_engine: None,
        }
    }

    pub fn run(&mut self) {
        self.show_welcome();
        
        loop {
            if let Some(session) = self.current_session.clone() {
                // In session context
                self.show_session_prompt(&session);
                if let Err(_) = self.handle_session_command(&session) {
                    break;
                }
            } else {
                // In main menu
                self.show_main_menu();
                if let Err(_) = self.handle_main_command() {
                    break;
                }
            }
        }
        
        println!("👋 Goodbye!");
    }

    fn show_welcome(&self) {
        println!("\n");
        println!("{}", "╔══════════════════════════════════════════════════════════════╗".cyan());
        println!("{}", "║                    🎯 WHISTLE PLAYGROUND                    ║".cyan());
        println!("{}", "║                 Interactive Trading System                   ║".cyan());
        println!("{}", "╚══════════════════════════════════════════════════════════════╝".cyan());
        println!();
        println!("{}", "🚀 Multi-Symbol Trading Engine with Real-Time Order Processing".yellow());
        println!("{}", "🔧 Built for Developers - Test, Monitor, and Validate".yellow());
        println!();
    }

    fn show_main_menu(&self) {
        println!();
        println!("{}", "╔══════════════════════════════════════════════════════════════╗".blue());
        println!("{}", "║                         MAIN MENU                           ║".blue());
        println!("{}", "╠══════════════════════════════════════════════════════════════╣".blue());
        println!("{}", "║  1. 📊 Sessions        - Manage trading sessions           ║".blue());
        println!("{}", "║  2. ➕ Create Session  - Start a new trading session       ║".blue());
        println!("{}", "║  3. 🚪 Exit           - Close the application              ║".blue());
        println!("{}", "╚══════════════════════════════════════════════════════════════╝".blue());
        println!();
    }

    fn show_session_prompt(&self, session: &str) {
        let account_info = if let Some(account) = self.current_account {
            format!(" (Account: {})", account)
        } else {
            " (No Account)".to_string()
        };
        
        print!("{}{}> ", session.cyan().bold(), account_info.yellow());
        io::stdout().flush().unwrap();
    }

    fn handle_main_command(&mut self) -> Result<(), ()> {
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();

        match input.as_str() {
            "1" | "sessions" => self.show_sessions(),
            "2" | "create" | "create session" => self.create_session_flow(),
            "3" | "exit" | "quit" => return Err(()),
            "help" => self.show_main_help(),
            _ => println!("❌ Invalid command. Type 'help' for available commands."),
        }
        Ok(())
    }

    fn handle_session_command(&mut self, session: &str) -> Result<(), ()> {
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();

        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "back" | "exit" => {
                self.exit_session();
                return Ok(());
            }
            "main" => {
                self.return_to_main();
                return Ok(());
            }
            "account" => {
                if parts.len() >= 2 {
                    if let Ok(account_id) = parts[1].parse::<u32>() {
                        self.switch_account(account_id);
                    } else {
                        println!("❌ Invalid account ID. Please enter a number.");
                    }
                } else {
                    println!("❌ Usage: account <account_id>");
                }
            }
            "submit" => {
                self.submit_order_flow(session);
                // Note: submit_order_flow now handles its own prompt redisplay
            }
            "symbols" | "list-symbols" => {
                self.list_symbols(session);
            }
            "status" => {
                self.show_session_status(session);
            }
            "help" => {
                self.show_session_help();
            }
            _ => {
                println!("❌ Unknown command. Type 'help' for available commands.");
            }
        }
        Ok(())
    }

    fn show_sessions(&mut self) {
        println!();
        println!("{}", "╔══════════════════════════════════════════════════════════════╗".green());
        println!("{}", "║                    📊 AVAILABLE SESSIONS                    ║".green());
        println!("{}", "╠══════════════════════════════════════════════════════════════╣".green());
        
        // For now, we'll use a simple approach to list sessions
        // In a real implementation, we'd scan the sessions directory
        println!("{}", "║  🎯 test-engine      - 4 symbols, 3 accounts               ║".green());
        println!("{}", "║  🏈 fantasy-league    - 8 symbols, 5 accounts               ║".green());
        println!("{}", "╚══════════════════════════════════════════════════════════════╝".green());
        println!();
        println!("{}", "💡 Commands:".yellow().bold());
        println!("  • Type session name to enter (e.g., 'fantasy-league')");
        println!("  • Type 'back' to return to main menu");
        println!("  • Type 'exit' to close application");
        println!();
        
        // Wait for user input to enter a session
        print!("{}", "🎯 Enter session name: ".cyan().bold());
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();
        
        match input {
            "back" => return,
            session_name => {
                if session_name == "test-engine" || session_name == "fantasy-league" {
                    self.enter_session(session_name);
                } else {
                    println!("❌ Unknown session: '{}'", session_name);
                    println!("💡 Available sessions: test-engine, fantasy-league");
                }
            }
        }
    }

    fn create_session_flow(&mut self) {
        print!("Session name: ");
        io::stdout().flush().unwrap();
        let mut name = String::new();
        io::stdin().read_line(&mut name).unwrap();
        let name = name.trim();

        print!("Number of accounts (default: 5): ");
        io::stdout().flush().unwrap();
        let mut accounts = String::new();
        io::stdin().read_line(&mut accounts).unwrap();
        let accounts: u32 = accounts.trim().parse().unwrap_or(5);

        print!("Number of symbols (default: 8): ");
        io::stdout().flush().unwrap();
        let mut symbols = String::new();
        io::stdin().read_line(&mut symbols).unwrap();
        let symbols: u32 = symbols.trim().parse().unwrap_or(8);

        match self.session_manager.create_session(name, accounts, symbols) {
            Ok(_) => {
                println!("✅ Session '{}' created successfully!", name);
                println!("📊 {} accounts, {} symbols", accounts, symbols);
            }
            Err(e) => {
                println!("❌ Failed to create session: {}", e);
            }
        }
    }

    fn enter_session(&mut self, session_name: &str) {
        match self.session_manager.load_session_config(session_name) {
            Ok(config) => {
                println!("🎯 Entering session: {}", session_name);
                println!("📊 Symbols: {}, Accounts: {}", config.symbols, config.accounts);
                
                // Show session overview
                self.show_session_overview(&config);
                
                // Create session directory path
                let sessions_dir = env::temp_dir().join("whistle-exchange");
                let session_dir = sessions_dir.join(session_name);
                
                // Create and start SessionEngine
                let mut engine = SessionEngine::new(session_name.to_string(), session_dir, config);
                
                match engine.start() {
                    Ok(_) => {
                        println!("🚀 SessionEngine started automatically");
                        self.session_engine = Some(engine);
                        self.current_session = Some(session_name.to_string());
                        
                        // Show available commands
                        println!("\n💡 Available commands: account, submit, symbols, status, help");
                        println!("💡 Type 'help' for detailed command information");
                        
                        // Add a small delay to ensure clean prompt display
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Err(e) => {
                        println!("❌ Failed to start SessionEngine: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("❌ Failed to load session: {}", e);
            }
        }
    }

    fn show_session_overview(&self, config: &crate::session::config::SessionConfig) {
        println!();
        println!("{}", "╔══════════════════════════════════════════════════════════════╗".magenta());
        println!("{}", "║                    📋 SESSION OVERVIEW                      ║".magenta());
        println!("{}", "╠══════════════════════════════════════════════════════════════╣".magenta());
        
        // Show top symbols
        println!("{}", "║  🏆 Top Symbols:                                              ║".magenta());
        let mut symbols: Vec<_> = config.symbols_info.iter().collect();
        symbols.sort_by_key(|(id, _)| *id);
        
        for (symbol_id, symbol_info) in symbols.iter().take(5) {
            let status = if symbol_info.active { "🟢" } else { "⚪" };
            println!("{}", format!("║    {} {} - {} ({})", status, symbol_id, symbol_info.name, symbol_info.position).magenta());
        }
        
        if config.symbols > 5 {
            println!("{}", format!("║    ... and {} more symbols", config.symbols - 5).magenta());
        }
        
        println!("{}", "║                                                              ║".magenta());
        println!("{}", format!("║  👥 Accounts: {} available", config.accounts).magenta());
        println!("{}", format!("║  📅 Created: {}", chrono::DateTime::from_timestamp(config.created as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "Unknown".to_string())).magenta());
        println!("{}", "╚══════════════════════════════════════════════════════════════╝".magenta());
        
        // Show trading status
        println!();
        println!("{}", "💹 Trading Status: Ready for orders".green().bold());
        println!("{}", "🎯 Use 'account <id>' to switch accounts, then 'submit' to place orders".yellow());
    }

    fn switch_account(&mut self, account_id: u32) {
        self.current_account = Some(account_id);
        println!("✅ Switched to Account {}", account_id);
    }

    fn submit_order_flow(&self, session: &str) {
        if self.current_account.is_none() {
            println!("❌ Please select an account first: account <account_id>");
            return;
        }

        let account_id = self.current_account.unwrap();
        
        print!("Side (buy/sell): ");
        io::stdout().flush().unwrap();
        let mut side = String::new();
        io::stdin().read_line(&mut side).unwrap();
        let side = side.trim().to_lowercase();

        // Get actual symbol count from session config
        let symbol_count = match self.session_manager.load_session_config(session) {
            Ok(config) => config.symbols,
            Err(_) => 8, // fallback
        };
        
        print!("Symbol ID (1-{}): ", symbol_count);
        io::stdout().flush().unwrap();
        let mut symbol_id = String::new();
        io::stdin().read_line(&mut symbol_id).unwrap();
        let symbol_id: u32 = symbol_id.trim().parse().unwrap_or(1);
        
        // Validate symbol ID
        if symbol_id < 1 || symbol_id > symbol_count {
            println!("❌ Invalid symbol ID. Must be between 1 and {}", symbol_count);
            return;
        }

        print!("Quantity: ");
        io::stdout().flush().unwrap();
        let mut qty = String::new();
        io::stdin().read_line(&mut qty).unwrap();
        let qty: u32 = qty.trim().parse().unwrap_or(100);

        print!("Price (optional, press Enter for market): ");
        io::stdout().flush().unwrap();
        let mut price = String::new();
        io::stdin().read_line(&mut price).unwrap();
        let price: Option<u32> = price.trim().parse().ok();

        // Generate order ID
        let order_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        match self.session_manager.submit_order_to_session_with_symbol(
            session,
            account_id,
            symbol_id,
            order_id,
            if side == "buy" { whistle::Side::Buy } else { whistle::Side::Sell },
            whistle::OrderType::Limit,
            price,
            qty,
        ) {
            Ok(_) => {
                println!("✅ Order submitted! Order ID: {}", order_id);
                println!("📝 {} {} {} @ {:?}", qty, side, symbol_id, price);
                
                // Add a small delay to ensure order processing messages are displayed
                std::thread::sleep(std::time::Duration::from_millis(100));
                
                // Note: Main loop will handle prompt redisplay
            }
            Err(e) => {
                println!("❌ Failed to submit order: {}", e);
                
                // Note: Main loop will handle prompt redisplay
            }
        }
    }

    fn list_symbols(&self, session: &str) {
        match self.session_manager.load_session_config(session) {
            Ok(config) => {
                println!("📊 Symbols in Session: {}", session);
                println!("Total Symbols: {}", config.symbols);
                println!();
                
                // Get active symbols from SessionEngine if available
                let active_symbols = if let Some(engine) = &self.session_engine {
                    engine.get_active_symbols().unwrap_or_default()
                } else {
                    Vec::new()
                };
                
                for (symbol_id, symbol_info) in &config.symbols_info {
                    let is_active = active_symbols.contains(symbol_id);
                    let status = if is_active { "🟢 Active" } else { "⚪ Inactive" };
                    let last_trade = if let Some(price) = symbol_info.last_trade_price {
                        format!(" (Last: ${})", price)
                    } else {
                        "".to_string()
                    };
                    
                    println!("  {} - {} ({}) - {}{}", 
                        symbol_id, symbol_info.name, symbol_info.position, status, last_trade);
                }
                
                // Show summary
                let active_count = active_symbols.len() as u32;
                println!("\n📈 Summary: {} active, {} inactive", active_count, config.symbols - active_count);
            }
            Err(e) => {
                println!("❌ Failed to load session config: {}", e);
            }
        }
    }

    fn show_session_status(&self, session: &str) {
        println!("📊 Session Status: {}", session);
        if let Some(account) = self.current_account {
            println!("👤 Current Account: {}", account);
        } else {
            println!("👤 Current Account: None (use 'account <id>' to switch)");
        }
        
        if let Some(engine) = &self.session_engine {
            if engine.is_running() {
                println!("🚀 SessionEngine: Running (Tick: {})", engine.get_current_tick());
            } else {
                println!("🚀 SessionEngine: Stopped");
            }
        } else {
            println!("🚀 SessionEngine: Not available");
        }
    }

    fn exit_session(&mut self) {
        if let Some(session) = &self.current_session {
            println!("👋 Exiting session: {}", session);
        }
        self.current_session = None;
        self.current_account = None;
        self.session_engine = None;
    }

    fn return_to_main(&mut self) {
        self.exit_session();
        println!("🏠 Returning to main menu...");
    }

    fn show_main_help(&self) {
        println!("\n{}", " Main Menu Commands".cyan().bold());
        println!("1, sessions    - List available sessions");
        println!("2, create      - Create a new session");
        println!("3, exit        - Exit the playground");
        println!("help           - Show this help");
        println!();
    }

    fn show_session_help(&self) {
        println!("\n{}", " Session Commands".cyan().bold());
        println!("account <id>   - Switch to account");
        println!("submit         - Submit a new order");
        println!("symbols        - List session symbols");
        println!("status         - Show session status");
        println!("back           - Exit session");
        println!("main           - Return to main menu");
        println!("help           - Show this help");
        println!();
    }
}
