// Event normalization for ExecutionManager

use crate::config::NormalizationConfig;
use crate::event::{
    BookDelta, DispatchEvent, ExecutionReport, LogLevel, OrderCancelled, OrderSubmitted, SystemLog,
    TickBoundaryEvent, TradeEvent,
};
use crate::id_allocator::ExecutionIdAllocator;
use std::time::Instant;
use whistle::{EngineEvent, LifecycleKind};

/// Event normalizer for converting Whistle events to ExecutionManager events
pub struct EventNormalizer {
    #[allow(dead_code)]
    config: NormalizationConfig,
}

impl EventNormalizer {
    pub fn new(config: NormalizationConfig) -> Self {
        Self { config }
    }

    pub fn normalize(
        &self,
        event: EngineEvent,
        id_allocator: &ExecutionIdAllocator,
    ) -> Result<DispatchEvent, String> {
        let now = Instant::now();

        match event {
            EngineEvent::Trade(trade) => {
                let execution_id = id_allocator.allocate_with_tick(trade.tick);

                // Create execution report
                let execution_report = ExecutionReport {
                    execution_id,
                    order_id: trade.taker_order,
                    price: trade.price,
                    quantity: trade.qty,
                    side: trade.taker_side,
                    aggressor_flag: true, // Taker is always aggressor
                    logical_timestamp: trade.tick,
                    wall_clock_timestamp: now,
                    symbol: trade.symbol,
                };

                // Create trade event
                let _trade_event = TradeEvent {
                    symbol: trade.symbol,
                    price: trade.price,
                    quantity: trade.qty,
                    aggressor_side: trade.taker_side,
                    logical_timestamp: trade.tick,
                    wall_clock_timestamp: now,
                    execution_id,
                };

                // Return both events (in practice, we'd dispatch both)
                Ok(DispatchEvent::ExecutionReport(execution_report))
            }

            EngineEvent::BookDelta(book_delta) => {
                let book_delta_event = BookDelta {
                    symbol: book_delta.symbol,
                    price_level: book_delta.price,
                    side: book_delta.side,
                    delta: book_delta.level_qty_after as i32, // Simplified - should calculate actual delta
                    new_quantity: book_delta.level_qty_after,
                    logical_timestamp: book_delta.tick,
                    wall_clock_timestamp: now,
                };

                Ok(DispatchEvent::BookDelta(book_delta_event))
            }

            EngineEvent::Lifecycle(lifecycle) => {
                match lifecycle.kind {
                    LifecycleKind::Cancelled => {
                        let order_cancelled = OrderCancelled {
                            order_id: lifecycle.order_id,
                            reason: lifecycle.reason.map(|r| format!("{r:?}")),
                            logical_timestamp: lifecycle.tick,
                            wall_clock_timestamp: now,
                            symbol: lifecycle.symbol,
                        };

                        Ok(DispatchEvent::OrderCancelled(order_cancelled))
                    }
                    LifecycleKind::Rejected => {
                        // Create system log for rejections
                        let system_log = SystemLog {
                            level: LogLevel::Warn,
                            message: format!(
                                "Order {} rejected: {:?}",
                                lifecycle.order_id, lifecycle.reason
                            ),
                            symbol: Some(lifecycle.symbol),
                            tick: Some(lifecycle.tick),
                            timestamp: now,
                        };

                        Ok(DispatchEvent::SystemLog(system_log))
                    }
                    LifecycleKind::Accepted => {
                        // Create order submission event for accepted orders
                        let order_submitted = OrderSubmitted {
                            order_id: lifecycle.order_id,
                            logical_timestamp: lifecycle.tick,
                            wall_clock_timestamp: now,
                            symbol: lifecycle.symbol,
                        };

                        Ok(DispatchEvent::OrderSubmitted(order_submitted))
                    }
                }
            }

            EngineEvent::TickComplete(tick_complete) => {
                let tick_boundary = TickBoundaryEvent {
                    tick: tick_complete.tick,
                    flushed_symbols: vec![tick_complete.symbol],
                    timestamp: now,
                    events_processed: 0, // Will be updated by ExecutionManager
                };

                Ok(DispatchEvent::TickBoundary(tick_boundary))
            }
        }
    }
}

/// Normalized event type
pub type NormalizedEvent = DispatchEvent;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NormalizationConfig;
    use whistle::{
        EvBookDelta, EvLifecycle, EvTickComplete, EvTrade, LifecycleKind, RejectReason, Side,
    };

    fn create_test_normalizer() -> EventNormalizer {
        EventNormalizer::new(NormalizationConfig::default())
    }

    #[test]
    fn test_trade_normalization() {
        let normalizer = create_test_normalizer();
        let id_allocator = ExecutionIdAllocator::new(Default::default());

        let trade = EngineEvent::Trade(EvTrade {
            symbol: 1,
            tick: 100,
            exec_id: 12345,
            price: 150,
            qty: 10,
            taker_side: Side::Buy,
            maker_order: 1,
            taker_order: 2,
        });

        let normalized = normalizer.normalize(trade, &id_allocator).unwrap();

        match normalized {
            DispatchEvent::ExecutionReport(report) => {
                assert_eq!(report.order_id, 2);
                assert_eq!(report.price, 150);
                assert_eq!(report.quantity, 10);
                assert_eq!(report.side, Side::Buy);
                assert!(report.aggressor_flag);
                assert_eq!(report.symbol, 1);
            }
            _ => panic!("Expected ExecutionReport"),
        }
    }

    #[test]
    fn test_book_delta_normalization() {
        let normalizer = create_test_normalizer();
        let id_allocator = ExecutionIdAllocator::new(Default::default());

        let book_delta = EngineEvent::BookDelta(EvBookDelta {
            symbol: 1,
            tick: 100,
            side: Side::Buy,
            price: 150,
            level_qty_after: 20,
        });

        let normalized = normalizer.normalize(book_delta, &id_allocator).unwrap();

        match normalized {
            DispatchEvent::BookDelta(delta) => {
                assert_eq!(delta.symbol, 1);
                assert_eq!(delta.price_level, 150);
                assert_eq!(delta.side, Side::Buy);
                assert_eq!(delta.new_quantity, 20);
            }
            _ => panic!("Expected BookDelta"),
        }
    }

    #[test]
    fn test_lifecycle_normalization() {
        let normalizer = create_test_normalizer();
        let id_allocator = ExecutionIdAllocator::new(Default::default());

        // Test cancellation
        let lifecycle = EngineEvent::Lifecycle(EvLifecycle {
            symbol: 1,
            tick: 100,
            kind: LifecycleKind::Cancelled,
            order_id: 123,
            reason: None,
        });

        let normalized = normalizer.normalize(lifecycle, &id_allocator).unwrap();

        match normalized {
            DispatchEvent::OrderCancelled(cancelled) => {
                assert_eq!(cancelled.order_id, 123);
                assert_eq!(cancelled.symbol, 1);
            }
            _ => panic!("Expected OrderCancelled"),
        }

        // Test rejection
        let lifecycle = EngineEvent::Lifecycle(EvLifecycle {
            symbol: 1,
            tick: 100,
            kind: LifecycleKind::Rejected,
            order_id: 124,
            reason: Some(RejectReason::BadTick),
        });

        let normalized = normalizer.normalize(lifecycle, &id_allocator).unwrap();

        match normalized {
            DispatchEvent::SystemLog(log) => {
                assert_eq!(log.level, LogLevel::Warn);
                assert!(log.message.contains("124"));
                assert!(log.message.contains("BadTick"));
            }
            _ => panic!("Expected SystemLog"),
        }
    }

    #[test]
    fn test_tick_complete_normalization() {
        let normalizer = create_test_normalizer();
        let id_allocator = ExecutionIdAllocator::new(Default::default());

        let tick_complete = EngineEvent::TickComplete(EvTickComplete { symbol: 1, tick: 100 });

        let normalized = normalizer.normalize(tick_complete, &id_allocator).unwrap();

        match normalized {
            DispatchEvent::TickBoundary(boundary) => {
                assert_eq!(boundary.tick, 100);
                assert_eq!(boundary.flushed_symbols, vec![1]);
            }
            _ => panic!("Expected TickBoundary"),
        }
    }
}
