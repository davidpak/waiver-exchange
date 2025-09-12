// Integration tests for SymbolCoordinator
// These tests demonstrate real functionality with multiple symbols and order processing

use crate::{CoordinatorConfig, SymbolCoordinator, SymbolCoordinatorApi};
use execution_manager::ExecutionManager;
use order_router::{
    InboundMsgWithSymbol, OrderRouter, RouterConfig,
    SymbolCoordinatorApi as OrderRouterCoordinatorApi,
};
use std::sync::Arc;
use whistle::{InboundMsg, OrderType, Side};

fn create_test_execution_manager() -> Arc<ExecutionManager> {
    Arc::new(ExecutionManager::new(execution_manager::ExecManagerConfig::default()))
}

/// Integration test that verifies the real flow from OrderRouter through SymbolCoordinator to Whistle
#[test]
fn test_real_order_processing_pipeline() {
    // Create SymbolCoordinator with test configuration
    let coord_config = CoordinatorConfig {
        num_threads: 2,
        spsc_depth: 64, // Small queue for testing
        max_symbols_per_thread: 4,
    };
    let execution_manager = create_test_execution_manager();
    let coordinator = SymbolCoordinator::new(coord_config, execution_manager);

    // Create OrderRouter
    let router_config = RouterConfig { spsc_depth_default: 64, ..Default::default() };
    let mut router = OrderRouter::new(router_config);

    // Wire the coordinator to the router
    let coordinator_box = Box::new(CoordinatorAdapter { coordinator });
    router.set_coordinator(coordinator_box);

    // Test 1: Symbol activation
    println!("🧪 Testing symbol activation...");
    let symbol_id = 1;

    // Initially, symbol should not be active
    assert!(!router.is_symbol_active(symbol_id));

    // Create a test order to trigger activation
    let test_msg = InboundMsgWithSymbol {
        symbol_id,
        msg: InboundMsg::submit(
            1001, // order_id
            1,    // account_id
            Side::Buy,
            OrderType::Limit,
            Some(150), // price
            10,        // qty
            1000,      // ts_norm
            0,         // meta
            0,         // enq_seq
        ),
    };

    // Route the order - this should activate the symbol
    let route_result = router.route(100, test_msg);
    assert!(route_result.is_ok(), "Order routing failed: {route_result:?}");

    // Now symbol should be active
    assert!(router.is_symbol_active(symbol_id), "Symbol should be active after first order");

    // Test 2: Order queuing and processing
    println!("🧪 Testing order queuing...");

    // Submit multiple orders to test queue behavior
    for i in 2..=5 {
        let msg = InboundMsgWithSymbol {
            symbol_id,
            msg: InboundMsg::submit(
                1000 + i, // order_id
                1,        // account_id
                Side::Buy,
                OrderType::Limit,
                Some((150 + i) as u32), // price
                10,                     // qty
                1000 + i,               // ts_norm
                0,                      // meta
                0,                      // enq_seq
            ),
        };

        let result = router.route(100, msg);
        assert!(result.is_ok(), "Order {i} routing failed: {result:?}");
    }

    // Test 3: Backpressure handling
    println!("🧪 Testing backpressure...");

    // Fill the queue to test backpressure
    let mut backpressure_count = 0;
    for i in 6..=100 {
        let msg = InboundMsgWithSymbol {
            symbol_id,
            msg: InboundMsg::submit(
                1000 + i, // order_id
                1,        // account_id
                Side::Buy,
                OrderType::Limit,
                Some((150 + i) as u32), // price
                10,                     // qty
                1000 + i,               // ts_norm
                0,                      // meta
                0,                      // enq_seq
            ),
        };

        match router.route(100, msg) {
            Ok(_) => {}
            Err(order_router::RouterError::Backpressure) => {
                backpressure_count += 1;
            }
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }

    // Should have hit backpressure at some point
    assert!(backpressure_count > 0, "Should have hit backpressure with small queue");
    println!("✅ Backpressure triggered {backpressure_count} times");

    // Test 4: Multiple symbols
    println!("🧪 Testing multiple symbols...");

    let symbol_2 = 2;
    let msg_symbol_2 = InboundMsgWithSymbol {
        symbol_id: symbol_2,
        msg: InboundMsg::submit(
            2001, // order_id
            1,    // account_id
            Side::Sell,
            OrderType::Limit,
            Some(200), // price
            5,         // qty
            2000,      // ts_norm
            0,         // meta
            0,         // enq_seq
        ),
    };

    let result = router.route(100, msg_symbol_2);
    assert!(result.is_ok(), "Symbol 2 order routing failed: {result:?}");
    assert!(router.is_symbol_active(symbol_2), "Symbol 2 should be active");

    // Test 5: Tick boundary behavior
    println!("🧪 Testing tick boundary behavior...");

    // Submit orders in different ticks
    let msg_tick_101 = InboundMsgWithSymbol {
        symbol_id,
        msg: InboundMsg::submit(
            3001, // order_id
            1,    // account_id
            Side::Sell,
            OrderType::Limit,
            Some(160), // price
            5,         // qty
            3000,      // ts_norm
            0,         // meta
            0,         // enq_seq
        ),
    };

    // Try to route the tick 101 order - it might fail due to backpressure, which is expected
    match router.route(101, msg_tick_101) {
        Ok(_) => println!("✅ Tick 101 order succeeded"),
        Err(order_router::RouterError::Backpressure) => {
            println!("⚠️  Tick 101 order hit backpressure (expected)")
        }
        Err(e) => panic!("Tick 101 order routing failed with unexpected error: {e:?}"),
    }

    // Test 6: Metrics verification
    println!("🧪 Testing metrics...");
    let metrics = router.metrics();

    assert!(metrics.active_symbols >= 2, "Should have at least 2 active symbols");
    assert!(metrics.enqueued > 0, "Should have enqueued orders");
    assert!(metrics.activation_requests >= 2, "Should have requested activation for 2 symbols");

    println!("✅ Integration test passed!");
    println!("📊 Final metrics: {metrics:?}");
}

/// Test that verifies the Whistle engine integration
#[test]
fn test_whistle_engine_integration() {
    println!("🧪 Testing Whistle engine integration...");

    // Create coordinator
    let coord_config =
        CoordinatorConfig { num_threads: 1, spsc_depth: 32, max_symbols_per_thread: 2 };
    let execution_manager = create_test_execution_manager();
    let coordinator = SymbolCoordinator::new(coord_config, execution_manager);

    // Test symbol activation and Whistle engine creation
    let symbol_id = 1;
    let result = coordinator.ensure_active(symbol_id);
    assert!(result.is_ok(), "Symbol activation failed: {result:?}");

    let ready_at = result.unwrap();
    assert_eq!(ready_at.next_tick, 0, "Should start at tick 0");

    // Verify the queue writer was created
    let queue_writer = &ready_at.queue_writer;
    assert!(queue_writer.queue.capacity() > 0, "Queue should have capacity");

    // Test order enqueuing directly to the queue using lock-free interface
    let test_msg = InboundMsg::submit(
        4001, // order_id
        1,    // account_id
        Side::Buy,
        OrderType::Limit,
        Some(150), // price
        10,        // qty
        4000,      // ts_norm
        0,         // meta
        0,         // enq_seq
    );

    let enqueue_result = queue_writer.queue.try_enqueue_lockfree(test_msg);
    assert!(enqueue_result.is_ok(), "Order enqueuing failed: {enqueue_result:?}");

    // Verify the order is in the queue
    assert!(!queue_writer.queue.is_empty(), "Queue should not be empty after enqueuing");
    assert_eq!(queue_writer.queue.len(), 1, "Queue should have 1 order");

    println!("✅ Whistle engine integration test passed!");
}

/// Test that verifies error handling and edge cases
#[test]
fn test_error_handling_and_edge_cases() {
    println!("🧪 Testing error handling and edge cases...");

    // Create coordinator with very small capacity
    let coord_config = CoordinatorConfig {
        num_threads: 1,
        spsc_depth: 2, // Very small queue
        max_symbols_per_thread: 1,
    };
    let execution_manager = create_test_execution_manager();
    let coordinator = SymbolCoordinator::new(coord_config, execution_manager);

    // Test 1: Activate symbol
    let symbol_id = 1;
    let result = coordinator.ensure_active(symbol_id);
    assert!(result.is_ok(), "Symbol activation failed: {result:?}");

    let ready_at = result.unwrap();
    let queue_writer = &ready_at.queue_writer;

    // Test 2: Queue backpressure with small capacity
    let mut backpressure_count = 0;
    for i in 1..=5 {
        let msg = InboundMsg::submit(
            5000 + i, // order_id
            1,        // account_id
            Side::Buy,
            OrderType::Limit,
            Some(150), // price
            10,        // qty
            5000 + i,  // ts_norm
            0,         // meta
            0,         // enq_seq
        );

        match queue_writer.queue.try_enqueue_lockfree(msg) {
            Ok(_) => {}
            Err(whistle::RejectReason::QueueBackpressure) => {
                backpressure_count += 1;
            }
            Err(e) => panic!("Unexpected reject reason: {e:?}"),
        }
    }

    assert!(backpressure_count > 0, "Should have hit backpressure with capacity 2");
    println!("✅ Backpressure test passed: {backpressure_count} rejections");

    // Test 3: Activate same symbol again (should succeed)
    let result2 = coordinator.ensure_active(symbol_id);
    assert!(result2.is_ok(), "Re-activating same symbol should succeed");

    // Test 4: Activate different symbol (should succeed)
    let symbol_id_2 = 2;
    let result3 = coordinator.ensure_active(symbol_id_2);
    assert!(result3.is_ok(), "Activating different symbol should succeed");

    println!("✅ Error handling and edge cases test passed!");
}

/// Adapter to bridge SymbolCoordinator to OrderRouter trait
struct CoordinatorAdapter {
    coordinator: SymbolCoordinator,
}

impl OrderRouterCoordinatorApi for CoordinatorAdapter {
    fn ensure_active(
        &self,
        symbol_id: u32,
    ) -> Result<order_router::ReadyAtTick, order_router::CoordError> {
        match self.coordinator.ensure_active(symbol_id) {
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
        self.coordinator.release_if_idle(symbol_id);
    }
}

/// Test that verifies the complete end-to-end flow
#[test]
fn test_complete_end_to_end_flow() {
    println!("🧪 Testing complete end-to-end flow...");

    // Create the complete system
    let coord_config =
        CoordinatorConfig { num_threads: 2, spsc_depth: 128, max_symbols_per_thread: 4 };
    let execution_manager = create_test_execution_manager();
    let coordinator = SymbolCoordinator::new(coord_config, execution_manager);

    let mut router = OrderRouter::new(RouterConfig::default());
    let coordinator_box = Box::new(CoordinatorAdapter { coordinator });
    router.set_coordinator(coordinator_box);

    // Test multiple symbols with different order types
    let test_cases = vec![
        (1, Side::Buy, OrderType::Limit, Some(150), 10),
        (2, Side::Sell, OrderType::Market, None, 5),
        (3, Side::Buy, OrderType::Ioc, Some(200), 15),
        (4, Side::Sell, OrderType::PostOnly, Some(180), 8),
    ];

    for (symbol_id, side, order_type, price, qty) in test_cases {
        let msg = InboundMsgWithSymbol {
            symbol_id,
            msg: InboundMsg::submit(
                (6000 + symbol_id) as u64, // order_id
                1,                         // account_id
                side,
                order_type,
                price,
                qty,
                (6000 + symbol_id) as u64, // ts_norm
                0,                         // meta
                0,                         // enq_seq
            ),
        };

        let result = router.route(100, msg);
        assert!(result.is_ok(), "Order for symbol {symbol_id} failed: {result:?}");

        // Verify symbol is active
        assert!(router.is_symbol_active(symbol_id), "Symbol {symbol_id} should be active");
    }

    // Verify all symbols are active
    let metrics = router.metrics();
    assert_eq!(metrics.active_symbols, 4, "Should have 4 active symbols");
    assert_eq!(metrics.enqueued, 4, "Should have enqueued 4 orders");

    println!("✅ Complete end-to-end flow test passed!");
    println!("📊 Final metrics: {metrics:?}");
}
