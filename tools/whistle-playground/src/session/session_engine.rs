use crate::session::config::SessionConfig;
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use symbol_coordinator::{CoordinatorConfig, SymbolCoordinator, SymbolCoordinatorApi as SymbolCoordinatorApiLocal};
use order_router::{OrderRouter, RouterConfig, InboundMsgWithSymbol, SymbolCoordinatorApi};
use whistle::{InboundMsg, OrderType, Side};

/// Adapter to bridge Arc<Mutex<SymbolCoordinator>> to the trait interface
struct CoordinatorAdapter {
    coordinator: Arc<Mutex<SymbolCoordinator>>,
}

impl SymbolCoordinatorApi for CoordinatorAdapter {
    fn ensure_active(&self, symbol_id: u32) -> Result<order_router::ReadyAtTick, order_router::CoordError> {
        let coordinator_guard = self.coordinator.lock()
            .map_err(|_| order_router::CoordError::Unknown)?;
        
        match coordinator_guard.ensure_active(symbol_id) {
            Ok(ready_at) => {
                // Convert from symbol_coordinator::ReadyAtTick to order_router::ReadyAtTick
                Ok(order_router::ReadyAtTick {
                    next_tick: ready_at.next_tick,
                    queue_writer: order_router::OrderQueueWriter {
                        queue: ready_at.queue_writer.queue.clone(),
                    },
                })
            }
            Err(_) => Err(order_router::CoordError::Unknown),
        }
    }

    fn release_if_idle(&self, symbol_id: u32) {
        if let Ok(coordinator_guard) = self.coordinator.lock() {
            coordinator_guard.release_if_idle(symbol_id);
        }
    }
}

pub struct SessionEngine {
    session_name: String,
    session_dir: PathBuf,
    coordinator: Arc<Mutex<SymbolCoordinator>>,
    router: Arc<Mutex<OrderRouter>>,
    config: SessionConfig,
    running: Arc<Mutex<bool>>,
    current_tick: Arc<Mutex<u64>>,
}

impl SessionEngine {
    pub fn new(session_name: String, session_dir: PathBuf, config: SessionConfig) -> Self {
        // Create SymbolCoordinator with appropriate configuration
        let coord_config = CoordinatorConfig {
            num_threads: 4,
            spsc_depth: 1024,
            max_symbols_per_thread: 16,
        };
        let coordinator = Arc::new(Mutex::new(SymbolCoordinator::new(coord_config)));
        
        // Create OrderRouter
        let router_config = RouterConfig::default();
        let mut router = OrderRouter::new(router_config);
        
        // Wire the coordinator to the router
        let coordinator_clone = coordinator.clone();
        let coordinator_box = Box::new(CoordinatorAdapter { 
            coordinator: coordinator_clone 
        });
        router.set_coordinator(coordinator_box);
        
        let router = Arc::new(Mutex::new(router));
        
        Self {
            session_name,
            session_dir,
            coordinator,
            router,
            config,
            running: Arc::new(Mutex::new(false)),
            current_tick: Arc::new(Mutex::new(100)),
        }
    }

    pub fn start(&mut self) -> Result<(), String> {
        println!("🚀 Starting SessionEngine for '{}'", self.session_name);
        println!("📊 Symbols: {}, Accounts: {}", self.config.symbols, self.config.accounts);
        
        let running = Arc::clone(&self.running);
        let mut running_guard = running.lock().map_err(|e| format!("Lock error: {}", e))?;
        *running_guard = true;
        drop(running_guard);
        
        // Start processing thread
        let session_dir = self.session_dir.clone();
        let coordinator = Arc::clone(&self.coordinator);
        let router = Arc::clone(&self.router);
        let running = Arc::clone(&self.running);
        let current_tick = Arc::clone(&self.current_tick);
        
        thread::spawn(move || {
            Self::process_orders_loop(
                session_dir,
                coordinator,
                router,
                running,
                current_tick,
            );
        });
        
        println!("✅ SessionEngine started successfully!");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        println!("🛑 Stopping SessionEngine for '{}'", self.session_name);
        
        let mut running_guard = self.running.lock().map_err(|e| format!("Lock error: {}", e))?;
        *running_guard = false;
        
        println!("✅ SessionEngine stopped");
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running.lock().map(|guard| *guard).unwrap_or(false)
    }

    pub fn get_current_tick(&self) -> u64 {
        self.current_tick.lock().map(|guard| *guard).unwrap_or(0)
    }

    pub fn get_active_symbols(&self) -> Result<Vec<u32>, String> {
        let _coordinator_guard = self.coordinator.lock().map_err(|e| format!("Lock error: {}", e))?;
        // For now, return all symbols as active since we're using placeholder implementation
        Ok((1..=self.config.symbols).collect())
    }

    fn process_orders_loop(
        session_dir: PathBuf,
        coordinator: Arc<Mutex<SymbolCoordinator>>,
        router: Arc<Mutex<OrderRouter>>,
        running: Arc<Mutex<bool>>,
        current_tick: Arc<Mutex<u64>>,
    ) {
        let mut last_processed_timestamp = 0u64;
        
        while *running.lock().unwrap() {
            // Read new orders from orders.jsonl
            if let Ok(orders) = Self::read_new_orders(&session_dir, last_processed_timestamp) {
                for order in &orders {
                    if let Err(e) = Self::process_order(&coordinator, &router, &current_tick, &session_dir, order.clone()) {
                        eprintln!("❌ Failed to process order: {}", e);
                    }
                }
                
                // Update last processed timestamp
                if let Some(last_order) = orders.last() {
                    if let Some(timestamp) = last_order.get("timestamp").and_then(|t| t.as_u64()) {
                        last_processed_timestamp = timestamp;
                    }
                }
            }
            
            // Advance tick
            if let Ok(mut tick_guard) = current_tick.lock() {
                *tick_guard += 1;
            }
            
            // Small delay to prevent busy waiting
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn read_new_orders(session_dir: &PathBuf, since_timestamp: u64) -> Result<Vec<Value>, String> {
        let orders_file = session_dir.join("orders.jsonl");
        if !orders_file.exists() {
            return Ok(Vec::new());
        }
        
        let content = fs::read_to_string(&orders_file)
            .map_err(|e| format!("Failed to read orders file: {}", e))?;
        
        let mut new_orders = Vec::new();
        for line in content.lines() {
            if let Ok(order_data) = serde_json::from_str::<Value>(line) {
                if let Some(timestamp) = order_data.get("timestamp").and_then(|t| t.as_u64()) {
                    if timestamp > since_timestamp {
                        new_orders.push(order_data);
                    }
                }
            }
        }
        
        Ok(new_orders)
    }

    fn process_order(
        coordinator: &Arc<Mutex<SymbolCoordinator>>,
        router: &Arc<Mutex<OrderRouter>>,
        current_tick: &Arc<Mutex<u64>>,
        session_dir: &PathBuf,
        order_data: Value,
    ) -> Result<(), String> {
        // Extract order details
        let symbol_id = order_data.get("symbol_id")
            .and_then(|s| s.as_u64())
            .ok_or("Missing symbol_id")? as u32;
        
        let account_id = order_data.get("account_id")
            .and_then(|a| a.as_u64())
            .ok_or("Missing account_id")? as u64;
        
        let order_id = order_data.get("order_id")
            .and_then(|o| o.as_u64())
            .ok_or("Missing order_id")?;
        
        let side = order_data.get("side")
            .and_then(|s| s.as_str())
            .ok_or("Missing side")?;
        
        let order_type = order_data.get("order_type")
            .and_then(|t| t.as_str())
            .ok_or("Missing order_type")?;
        
        let price = order_data.get("price").and_then(|p| p.as_u64());
        let qty = order_data.get("qty")
            .and_then(|q| q.as_u64())
            .ok_or("Missing qty")? as u32;
        
        let ts_norm = order_data.get("timestamp")
            .and_then(|t| t.as_u64())
            .unwrap_or_else(|| SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64);
        
        // Convert to Whistle types
        let side_enum = match side {
            "buy" => Side::Buy,
            "sell" => Side::Sell,
            _ => return Err(format!("Invalid side: {}", side)),
        };
        
        let order_type_enum = match order_type {
            "limit" => OrderType::Limit,
            "market" => OrderType::Market,
            "ioc" => OrderType::Ioc,
            "post-only" => OrderType::PostOnly,
            _ => return Err(format!("Invalid order type: {}", order_type)),
        };
        
        // Ensure symbol is active
        let coordinator_guard = coordinator.lock().map_err(|e| format!("Lock error: {}", e))?;
        let _ready_at = coordinator_guard.ensure_active(symbol_id)
            .map_err(|e| format!("Failed to activate symbol {}: {:?}", symbol_id, e))?;
        drop(coordinator_guard);
        
        // Create InboundMsg
        let msg = InboundMsg::submit(
            order_id,
            account_id,
            side_enum,
            order_type_enum,
            price.map(|p| p as u32),
            qty,
            ts_norm,
            0, // meta
            0, // enq_seq (will be stamped by router)
        );
        
        // Create InboundMsgWithSymbol
        let msg_with_symbol = InboundMsgWithSymbol {
            symbol_id,
            msg,
        };
        
        // Route the order
        let tick = *current_tick.lock().map_err(|e| format!("Lock error: {}", e))?;
        let mut router_guard = router.lock().map_err(|e| format!("Lock error: {}", e))?;
        let _result = router_guard.route(tick, msg_with_symbol)
            .map_err(|e| format!("Failed to route order: {:?}", e))?;
        
        // Process the real Whistle engine for this symbol
        if let Ok(mut coordinator_guard) = coordinator.lock() {
            if let Some(events) = coordinator_guard.process_symbol_tick(symbol_id, tick) {
                // Write events to files for monitor compatibility
                if !events.is_empty() {
                    if let Err(e) = Self::write_events_to_files(&session_dir, symbol_id, events) {
                        eprintln!("❌ Failed to write events to file: {}", e);
                    }
                }
            }
        }
        
        println!("✅ Processed order {} for symbol {} ({} {} @ {:?})", 
            order_id, symbol_id, qty, side, price);
        
        Ok(())
    }

    /// Write Whistle engine events to files for monitor compatibility
    fn write_events_to_files(
        session_dir: &PathBuf,
        symbol_id: u32,
        events: Vec<whistle::EngineEvent>,
    ) -> Result<(), String> {
        let trades_file = session_dir.join("trades.jsonl");
        let book_file = session_dir.join("book_updates.jsonl");
        
        let mut trades_writer = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&trades_file)
            .map_err(|e| format!("Failed to open trades file: {}", e))?;
        
        let mut book_writer = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&book_file)
            .map_err(|e| format!("Failed to open book file: {}", e))?;
        
        for event in events {
            match event {
                whistle::EngineEvent::Trade(ev) => {
                    let trade_data = serde_json::json!({
                        "symbol_id": symbol_id,
                        "price": ev.price,
                        "qty": ev.qty,
                        "side": format!("{:?}", ev.taker_side),
                        "timestamp": ev.tick,
                        "exec_id": ev.exec_id,
                        "maker_order": ev.maker_order,
                        "taker_order": ev.taker_order,
                    });
                    
                    writeln!(trades_writer, "{}", trade_data)
                        .map_err(|e| format!("Failed to write trade: {}", e))?;
                }
                
                whistle::EngineEvent::BookDelta(ev) => {
                    let book_data = serde_json::json!({
                        "symbol_id": symbol_id,
                        "side": format!("{:?}", ev.side),
                        "price": ev.price,
                        "level_qty_after": ev.level_qty_after,
                        "timestamp": ev.tick,
                    });
                    
                    writeln!(book_writer, "{}", book_data)
                        .map_err(|e| format!("Failed to write book update: {}", e))?;
                }
                
                // Handle other event types as needed
                _ => {}
            }
        }
        
        Ok(())
    }
}
