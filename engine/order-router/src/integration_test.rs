use crate::{OrderRouter, RouterConfig, RouterError, SymbolCoordinatorApi};
use execution_manager::ExecutionManager;
use std::sync::Arc;
use symbol_coordinator::{
    CoordinatorConfig, SymbolCoordinator, SymbolCoordinatorApi as SymbolCoordinatorApiLocal,
};
use whistle::{InboundMsg, OrderType, Side};

fn create_test_execution_manager() -> Arc<ExecutionManager> {
    Arc::new(ExecutionManager::new(execution_manager::ExecManagerConfig::default()))
}

/// Integration test that verifies OrderRouter working with real SymbolCoordinator
#[test]
fn test_order_router_symbol_coordinator_integration() {
    println!("🧪 Testing OrderRouter + SymbolCoordinator integration...");

    // Create SymbolCoordinator
    let coord_config =
        CoordinatorConfig { num_threads: 2, spsc_depth: 64, max_symbols_per_thread: 4 };
    let execution_manager = create_test_execution_manager();
    let coordinator = SymbolCoordinator::new(coord_config, execution_manager);

    // Create OrderRouter
    let router_config = RouterConfig { spsc_depth_default: 64, ..Default::default() };
    let mut router = OrderRouter::new(router_config);

    // Wire them together
    let coordinator_box = Box::new(CoordinatorAdapter { coordinator });
    router.set_coordinator(coordinator_box);

    // Test 1: Symbol activation through order routing
    println!("🧪 Testing symbol activation through order routing...");

    let symbol_id = 1;
    assert!(!router.is_symbol_active(symbol_id), "Symbol should not be active initially");

    // Submit first order - should activate symbol
    let first_order =
        create_test_message(symbol_id, 1001, Side::Buy, OrderType::Limit, Some(150), 10);
    let result = router.route(100, first_order);
    assert!(result.is_ok(), "First order should succeed and activate symbol");

    assert!(router.is_symbol_active(symbol_id), "Symbol should be active after first order");

    // Test 2: Multiple orders to same symbol
    println!("🧪 Testing multiple orders to same symbol...");

    for i in 2..=5 {
        let order = create_test_message(
            symbol_id,
            1000 + i,
            Side::Buy,
            OrderType::Limit,
            Some((150 + i) as u32),
            10,
        );
        let result = router.route(100, order);
        assert!(result.is_ok(), "Order {i} should succeed");
    }

    // Test 3: Multiple symbols
    println!("🧪 Testing multiple symbols...");

    let symbols = vec![2, 3, 4];
    for &symbol_id in &symbols {
        let order = create_test_message(
            symbol_id,
            (2000 + symbol_id) as u64,
            Side::Sell,
            OrderType::Limit,
            Some(200 + symbol_id),
            5,
        );
        let result = router.route(100, order);
        assert!(result.is_ok(), "Order for symbol {symbol_id} should succeed");
        assert!(router.is_symbol_active(symbol_id), "Symbol {symbol_id} should be active");
    }

    // Test 4: Different order types
    println!("🧪 Testing different order types...");

    let order_types = vec![
        (OrderType::Market, None),
        (OrderType::Ioc, Some(160)),
        (OrderType::PostOnly, Some(170)),
    ];

    for (order_type, price) in order_types {
        let order = create_test_message(5, 3000, Side::Buy, order_type, price, 15);
        let result = router.route(100, order);
        assert!(result.is_ok(), "{order_type:?} order should succeed");
    }

    // Test 5: Tick boundary behavior
    println!("🧪 Testing tick boundary behavior...");

    let tick_101_order = create_test_message(1, 4001, Side::Sell, OrderType::Limit, Some(160), 5);
    let result = router.route(101, tick_101_order);
    assert!(result.is_ok(), "Tick 101 order should succeed");

    // Test 6: Metrics verification
    println!("🧪 Testing metrics...");
    let metrics = router.metrics();

    assert_eq!(metrics.active_symbols, 5, "Should have 5 active symbols");
    assert!(metrics.enqueued >= 10, "Should have enqueued at least 10 orders");
    assert_eq!(metrics.activation_requests, 5, "Should have requested activation for 5 symbols");

    println!("✅ OrderRouter + SymbolCoordinator integration test passed!");
    println!("📊 Final metrics: {metrics:?}");
}

/// Test that verifies queue behavior and backpressure
#[test]
fn test_queue_behavior_and_backpressure() {
    println!("🧪 Testing queue behavior and backpressure...");

    // Create system with small queue capacity
    let coord_config = CoordinatorConfig {
        num_threads: 1,
        spsc_depth: 4, // Very small queue
        max_symbols_per_thread: 2,
    };
    let execution_manager = create_test_execution_manager();
    let coordinator = SymbolCoordinator::new(coord_config, execution_manager);

    let mut router = OrderRouter::new(RouterConfig { spsc_depth_default: 4, ..Default::default() });

    let coordinator_box = Box::new(CoordinatorAdapter { coordinator });
    router.set_coordinator(coordinator_box);

    let symbol_id = 1;

    // Fill the queue to capacity
    let mut successful_orders = 0;
    let mut backpressure_errors = 0;

    for i in 1..=10 {
        let order = create_test_message(
            symbol_id,
            5000 + i,
            Side::Buy,
            OrderType::Limit,
            Some((150 + i) as u32),
            10,
        );

        match router.route(100, order) {
            Ok(_) => successful_orders += 1,
            Err(RouterError::Backpressure) => backpressure_errors += 1,
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }

    // Should have hit backpressure
    assert!(backpressure_errors > 0, "Should have hit backpressure with small queue");
    assert!(successful_orders > 0, "Should have successfully queued some orders");

    println!("✅ Queue behavior test passed!");
    println!("📊 Successful: {successful_orders}, Backpressure: {backpressure_errors}");
}

/// Test that verifies error handling scenarios
#[test]
fn test_error_handling_scenarios() {
    println!("🧪 Testing error handling scenarios...");

    let coord_config =
        CoordinatorConfig { num_threads: 1, spsc_depth: 16, max_symbols_per_thread: 1 };
    let execution_manager = create_test_execution_manager();
    let coordinator = SymbolCoordinator::new(coord_config, execution_manager);

    let mut router = OrderRouter::new(RouterConfig::default());
    let coordinator_box = Box::new(CoordinatorAdapter { coordinator });
    router.set_coordinator(coordinator_box);

    // Test 1: Order to inactive symbol (should activate automatically)
    let symbol_id = 1;
    let order = create_test_message(symbol_id, 6001, Side::Buy, OrderType::Limit, Some(150), 10);
    let result = router.route(100, order);
    assert!(result.is_ok(), "Order to inactive symbol should succeed and activate it");

    // Test 2: Multiple orders to same symbol (should all succeed)
    for i in 2..=5 {
        let order = create_test_message(
            symbol_id,
            6000 + i,
            Side::Buy,
            OrderType::Limit,
            Some((150 + i) as u32),
            10,
        );
        let result = router.route(100, order);
        assert!(result.is_ok(), "Order {i} to active symbol should succeed");
    }

    // Test 3: Order to different symbol (should activate automatically)
    let symbol_id_2 = 2;
    let order = create_test_message(symbol_id_2, 7001, Side::Sell, OrderType::Limit, Some(200), 5);
    let result = router.route(100, order);
    assert!(result.is_ok(), "Order to new symbol should succeed and activate it");

    println!("✅ Error handling scenarios test passed!");
}

/// Test that verifies performance characteristics
#[test]
fn test_performance_characteristics() {
    println!("🧪 Testing performance characteristics...");

    let coord_config =
        CoordinatorConfig { num_threads: 4, spsc_depth: 1024, max_symbols_per_thread: 16 };
    let execution_manager = create_test_execution_manager();
    let coordinator = SymbolCoordinator::new(coord_config, execution_manager);

    let mut router =
        OrderRouter::new(RouterConfig { spsc_depth_default: 1024, ..Default::default() });

    let coordinator_box = Box::new(CoordinatorAdapter { coordinator });
    router.set_coordinator(coordinator_box);

    // Test high-volume order processing
    let num_symbols = 10;
    let orders_per_symbol = 50;

    let start_time = std::time::Instant::now();

    for symbol_id in 1..=num_symbols {
        for order_num in 1..=orders_per_symbol {
            let order = create_test_message(
                symbol_id,
                (8000 + symbol_id * 1000 + order_num) as u64,
                Side::Buy,
                OrderType::Limit,
                Some(150 + order_num),
                10,
            );

            let result = router.route(100, order);
            assert!(result.is_ok(), "High-volume order should succeed");
        }
    }

    let duration = start_time.elapsed();
    let total_orders = num_symbols * orders_per_symbol;
    let orders_per_second = total_orders as f64 / duration.as_secs_f64();

    println!("✅ Performance test passed!");
    println!("📊 Processed {total_orders} orders in {duration:?}");
    println!("📊 Rate: {orders_per_second:.2} orders/second");

    // Verify all symbols are active
    let metrics = router.metrics();
    assert_eq!(metrics.active_symbols, num_symbols, "All symbols should be active");
    assert_eq!(metrics.enqueued, total_orders as u64, "All orders should be enqueued");
}

/// Helper function to create test messages
fn create_test_message(
    symbol_id: u32,
    order_id: u64,
    side: Side,
    order_type: OrderType,
    price: Option<u32>,
    qty: u64,
) -> crate::InboundMsgWithSymbol {
    crate::InboundMsgWithSymbol {
        symbol_id,
        msg: InboundMsg::submit(
            order_id, 1, // account_id
            side, order_type, price, qty, order_id, // ts_norm
            0,        // meta
            0,        // enq_seq
        ),
    }
}

/// Adapter to bridge SymbolCoordinator to OrderRouter trait
struct CoordinatorAdapter {
    coordinator: SymbolCoordinator,
}

impl SymbolCoordinatorApi for CoordinatorAdapter {
    fn ensure_active(&self, symbol_id: u32) -> Result<crate::ReadyAtTick, crate::CoordError> {
        match self.coordinator.ensure_active(symbol_id) {
            Ok(ready_at) => {
                // Convert from symbol_coordinator::ReadyAtTick to order_router::ReadyAtTick
                Ok(crate::ReadyAtTick {
                    next_tick: ready_at.next_tick,
                    queue_writer: crate::OrderQueueWriter {
                        queue: ready_at.queue_writer.queue.clone(),
                    },
                })
            }
            Err(_) => Err(crate::CoordError::Unknown),
        }
    }

    fn release_if_idle(&self, symbol_id: u32) {
        self.coordinator.release_if_idle(symbol_id);
    }
}
