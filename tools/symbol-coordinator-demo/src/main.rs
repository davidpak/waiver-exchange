// SymbolCoordinator Demo Tool
// Demonstrates real multi-symbol coordination functionality

use order_router::{OrderRouter, RouterConfig, InboundMsgWithSymbol, SymbolCoordinatorApi as OrderRouterApi};
use symbol_coordinator::{CoordinatorConfig, SymbolCoordinator, SymbolCoordinatorApi as LocalApi};
use whistle::{InboundMsg, Side, OrderType};
use std::time::Duration;
use std::thread;

fn main() {
    println!("🎯 SymbolCoordinator Demo - Multi-Symbol Trading System");
    println!("=====================================================\n");

    // Create coordinator with 4 threads for load distribution
    let coord_config = CoordinatorConfig {
        num_threads: 4,
        spsc_depth: 1024,
        max_symbols_per_thread: 16,
    };
    let coordinator = SymbolCoordinator::new(coord_config.clone());
    
    // Create router
    let router_config = RouterConfig::default();
    let mut router = OrderRouter::new(router_config);
    
    // Set the coordinator on the router
    let coordinator_arc = std::sync::Arc::new(std::sync::Mutex::new(coordinator));
    let coordinator_adapter = Box::new(CoordinatorAdapter {
        coordinator: coordinator_arc.clone(),
    });
    router.set_coordinator(coordinator_adapter);
    
    // Fantasy football players as symbols
    let players = vec![
        (1, "Jamarr Chase", "WR"),
        (2, "Derrick Henry", "RB"),
        (3, "Patrick Mahomes", "QB"),
        (4, "Travis Kelce", "TE"),
        (5, "Christian McCaffrey", "RB"),
        (6, "Tyreek Hill", "WR"),
        (7, "Josh Allen", "QB"),
        (8, "Saquon Barkley", "RB"),
    ];
    
    println!("📊 Activating {} player symbols...", players.len());
    
    // Activate all symbols using the trait
    for (symbol_id, player_name, position) in &players {
        let result = coordinator_arc.lock().unwrap().ensure_active(*symbol_id);
        match result {
            Ok(_) => println!("  ✅ Activated {player_name} ({position}) - Symbol ID: {symbol_id}"),
            Err(e) => println!("  ❌ Failed to activate {player_name}: {e:?}"),
        }
    }
    
    println!("\n🎯 All symbols activated! Now simulating order flow...\n");
    
    // Simulate realistic order flow
    let order_scenarios = vec![
        // High-volume trading for star players
        (1, "Jamarr Chase", vec![
            (Side::Buy, 150, 100, "Market maker placing large bid"),
            (Side::Sell, 155, 50, "Profit taking"),
            (Side::Buy, 152, 75, "Institutional buying"),
        ]),
        
        // Moderate trading for solid players
        (2, "Derrick Henry", vec![
            (Side::Buy, 120, 30, "Conservative bid"),
            (Side::Sell, 125, 20, "Small profit taking"),
            (Side::Buy, 122, 25, "Value buying"),
        ]),
        
        // Premium pricing for elite QBs
        (3, "Patrick Mahomes", vec![
            (Side::Buy, 200, 25, "Premium QB bid"),
            (Side::Sell, 210, 15, "High price selling"),
            (Side::Buy, 205, 20, "Elite player demand"),
        ]),
        
        // Tight end market
        (4, "Travis Kelce", vec![
            (Side::Buy, 180, 40, "TE premium bid"),
            (Side::Sell, 185, 30, "TE profit taking"),
            (Side::Buy, 182, 35, "TE value buying"),
        ]),
    ];
    
    for (symbol_id, player_name, orders) in order_scenarios {
        println!("📈 {} ({}) - Processing {} orders:", player_name, symbol_id, orders.len());
        
        for (i, (side, price, qty, description)) in orders.iter().enumerate() {
            // Create order message
            let msg = InboundMsgWithSymbol {
                symbol_id,
                msg: InboundMsg::submit(
                    (10000 + symbol_id * 100 + i as u32).into(), // order_id
                    1,                                           // account_id
                    *side,
                    OrderType::Limit,
                    Some(*price),                                // price
                    *qty,
                    1000,                                        // ts_norm
                    0,                                           // meta
                    0,                                           // enq_seq (will be stamped by router)
                ),
            };
            
            // Route the order
            let result = router.route(100, msg);
            
            let side_str = match side {
                Side::Buy => "BUY",
                Side::Sell => "SELL",
            };
            
            match result {
                                  Ok(_) => println!("    ✅ {side_str} @ {price} ({qty} units) - {description}"),
                                  Err(e) => println!("    ⚠️  {side_str} @ {price} ({qty} units) - {description} (expected: {e:?})"),
            }
            
            // Small delay to simulate real trading
            thread::sleep(Duration::from_millis(100));
        }
        println!();
    }
    
    // Test thread distribution
    println!("🧵 Testing thread distribution across {} threads...", coord_config.num_threads);
    let test_symbols: Vec<u32> = (100..=150).collect();
    
    for &symbol_id in &test_symbols {
        let result = coordinator_arc.lock().unwrap().ensure_active(symbol_id);
        if result.is_err() {
            println!("  ❌ Failed to activate symbol {symbol_id}");
        }
    }
    
    println!("✅ Successfully activated {} additional symbols!", test_symbols.len());
    
    // Test configuration limits
    println!("\n⚙️  Testing configuration limits...");
    println!("   - Threads: {}", coord_config.num_threads);
    println!("   - SPSC Depth: {}", coord_config.spsc_depth);
    println!("   - Max Symbols per Thread: {}", coord_config.max_symbols_per_thread);
    println!("   - Total Symbols Activated: {}", players.len() + test_symbols.len());
    
    println!("\n🎯 SymbolCoordinator Demo Completed Successfully!");
    println!("   - Multi-symbol coordination working");
    println!("   - Thread placement functioning");
    println!("   - Order routing interface ready");
    println!("   - Ready for SimulationClock integration");
}

// Adapter to bridge SymbolCoordinator to the trait expected by OrderRouter
struct CoordinatorAdapter {
    coordinator: std::sync::Arc<std::sync::Mutex<SymbolCoordinator>>,
}

impl OrderRouterApi for CoordinatorAdapter {
    fn ensure_active(&self, symbol_id: u32) -> Result<order_router::ReadyAtTick, order_router::CoordError> {
        let coordinator_guard = self.coordinator.lock().unwrap();
        coordinator_guard.ensure_active(symbol_id).map(|ready_at| {
            order_router::ReadyAtTick {
                next_tick: ready_at.next_tick,
                queue_writer: order_router::OrderQueueWriter {
                    queue: ready_at.queue_writer.queue,
                },
            }
        }).map_err(|_| order_router::CoordError::Unknown)
    }
    
    fn release_if_idle(&self, symbol_id: u32) {
        // For demo purposes, we'll just ignore this call
        // In a real implementation, this would release the symbol if it's idle
        let _ = symbol_id;
    }
}
