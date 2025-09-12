//! Analytics event conversion for ExecutionManager
//!
//! Converts ExecutionManager events to AnalyticsEngine events for monitoring and analytics.

use crate::event::{
    BookDelta, DispatchEvent, ExecutionReport, LogLevel, OrderCancelled, OrderSubmitted, SystemLog,
    TickBoundaryEvent, TradeEvent,
};
use analytics_engine::analytics::{
    AnalyticsEvent, BusinessMetrics, EventType, OperationalMetrics, PerformanceMetrics,
    SystemHealthMetrics,
};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Converts ExecutionManager events to AnalyticsEngine events
pub struct AnalyticsConverter {
    /// Sampling configuration
    sampling_interval_ticks: u64,
    last_sampled_tick: AtomicU64, // 0 means None

    /// Performance tracking
    events_in_tick: AtomicU32,
}

impl AnalyticsConverter {
    /// Create new analytics converter
    pub fn new(sampling_interval_ticks: u64) -> Self {
        Self {
            sampling_interval_ticks,
            last_sampled_tick: AtomicU64::new(0), // 0 means None
            events_in_tick: AtomicU32::new(0),
        }
    }

    /// Convert ExecutionManager event to AnalyticsEngine event
    pub fn convert_event(&self, event: &DispatchEvent) -> Option<AnalyticsEvent> {
        match event {
            DispatchEvent::ExecutionReport(report) => {
                self.events_in_tick.fetch_add(1, Ordering::Relaxed);
                Some(self.convert_execution_report(report))
            }
            DispatchEvent::TradeEvent(trade) => {
                self.events_in_tick.fetch_add(1, Ordering::Relaxed);
                Some(self.convert_trade_event(trade))
            }
            DispatchEvent::OrderSubmitted(submitted) => {
                self.events_in_tick.fetch_add(1, Ordering::Relaxed);
                Some(self.convert_order_submitted(submitted))
            }
            DispatchEvent::OrderCancelled(cancel) => {
                self.events_in_tick.fetch_add(1, Ordering::Relaxed);
                Some(self.convert_order_cancelled(cancel))
            }
            DispatchEvent::BookDelta(delta) => {
                self.events_in_tick.fetch_add(1, Ordering::Relaxed);
                Some(self.convert_book_delta(delta))
            }
            DispatchEvent::TickBoundary(boundary) => self.handle_tick_boundary(boundary),
            DispatchEvent::SystemLog(log) => {
                self.events_in_tick.fetch_add(1, Ordering::Relaxed);
                Some(self.convert_system_log(log))
            }
        }
    }

    /// Convert execution report to business metrics
    fn convert_execution_report(&self, report: &ExecutionReport) -> AnalyticsEvent {
        AnalyticsEvent {
            timestamp_ns: current_timestamp_ns(),
            tick_id: report.logical_timestamp,
            symbol: format!("SYMBOL_{}", report.symbol),
            event_type: EventType::Business as i32,
            data: Some(analytics_engine::analytics::analytics_event::Data::Business(
                BusinessMetrics {
                    orders_processed: 1,
                    trades_executed: 1,
                    volume_traded: report.quantity as u64,
                    active_accounts: 1,  // Would need account tracking
                    order_book_depth: 0, // Would need book state
                    average_trade_size: report.quantity as f64,
                },
            )),
        }
    }

    /// Convert trade event to business metrics
    fn convert_trade_event(&self, trade: &TradeEvent) -> AnalyticsEvent {
        AnalyticsEvent {
            timestamp_ns: current_timestamp_ns(),
            tick_id: trade.logical_timestamp,
            symbol: format!("SYMBOL_{}", trade.symbol),
            event_type: EventType::Business as i32,
            data: Some(analytics_engine::analytics::analytics_event::Data::Business(
                BusinessMetrics {
                    orders_processed: 0,
                    trades_executed: 1,
                    volume_traded: trade.quantity as u64,
                    active_accounts: 1,
                    order_book_depth: 0,
                    average_trade_size: trade.quantity as f64,
                },
            )),
        }
    }

    /// Convert order submitted to business metrics
    fn convert_order_submitted(&self, submitted: &OrderSubmitted) -> AnalyticsEvent {
        AnalyticsEvent {
            timestamp_ns: current_timestamp_ns(),
            tick_id: submitted.logical_timestamp,
            symbol: format!("SYMBOL_{}", submitted.symbol),
            event_type: EventType::Business as i32,
            data: Some(analytics_engine::analytics::analytics_event::Data::Business(
                BusinessMetrics {
                    orders_processed: 1,
                    trades_executed: 0,
                    volume_traded: 0,
                    active_accounts: 1,
                    order_book_depth: 0,
                    average_trade_size: 0.0,
                },
            )),
        }
    }

    /// Convert order cancelled to business metrics
    fn convert_order_cancelled(&self, cancel: &OrderCancelled) -> AnalyticsEvent {
        AnalyticsEvent {
            timestamp_ns: current_timestamp_ns(),
            tick_id: cancel.logical_timestamp,
            symbol: format!("SYMBOL_{}", cancel.symbol),
            event_type: EventType::Business as i32,
            data: Some(analytics_engine::analytics::analytics_event::Data::Business(
                BusinessMetrics {
                    orders_processed: 1,
                    trades_executed: 0,
                    volume_traded: 0,
                    active_accounts: 1,
                    order_book_depth: 0,
                    average_trade_size: 0.0,
                },
            )),
        }
    }

    /// Convert book delta to operational metrics
    fn convert_book_delta(&self, delta: &BookDelta) -> AnalyticsEvent {
        AnalyticsEvent {
            timestamp_ns: current_timestamp_ns(),
            tick_id: delta.logical_timestamp,
            symbol: format!("SYMBOL_{}", delta.symbol),
            event_type: EventType::Operational as i32,
            data: Some(analytics_engine::analytics::analytics_event::Data::Operational(
                OperationalMetrics {
                    symbol_activated: false,
                    symbol_evicted: false,
                    thread_utilization_percent: 0.0, // Would need system metrics
                    network_bytes_sent: 0,
                    disk_bytes_written: 0,
                    active_symbols: 1,
                },
            )),
        }
    }

    /// Convert system log to health metrics
    fn convert_system_log(&self, log: &SystemLog) -> AnalyticsEvent {
        AnalyticsEvent {
            timestamp_ns: current_timestamp_ns(),
            tick_id: log.tick.unwrap_or(0),
            symbol: log.symbol.map(|s| format!("SYMBOL_{s}")).unwrap_or_default(),
            event_type: EventType::SystemHealth as i32,
            data: Some(analytics_engine::analytics::analytics_event::Data::Health(
                SystemHealthMetrics {
                    engine_crashed: log.level == LogLevel::Error && log.message.contains("crash"),
                    queue_overflows: if log.message.contains("overflow") { 1 } else { 0 },
                    memory_allocation_failures: if log.message.contains("memory") { 1 } else { 0 },
                    error_rate_percent: if log.level == LogLevel::Error { 1.0 } else { 0.0 },
                    uptime_seconds: 0, // Would need uptime tracking
                    error_message: log.message.clone(),
                },
            )),
        }
    }

    /// Handle tick boundary events and emit performance metrics
    fn handle_tick_boundary(&self, boundary: &TickBoundaryEvent) -> Option<AnalyticsEvent> {
        let current_tick = boundary.tick;

        // Check if we should sample this tick
        let last_tick = self.last_sampled_tick.load(Ordering::Relaxed);
        let should_sample = if last_tick == 0 {
            true
        } else {
            current_tick - last_tick >= self.sampling_interval_ticks
        };

        if should_sample {
            self.last_sampled_tick.store(current_tick, Ordering::Relaxed);

            // Calculate tick duration (simplified - no timing for now)
            let tick_duration = 0;
            let events_count = self.events_in_tick.load(Ordering::Relaxed);
            self.events_in_tick.store(0, Ordering::Relaxed);

            Some(AnalyticsEvent {
                timestamp_ns: current_timestamp_ns(),
                tick_id: current_tick,
                symbol: "SYSTEM".to_string(),
                event_type: EventType::Performance as i32,
                data: Some(analytics_engine::analytics::analytics_event::Data::Performance(
                    PerformanceMetrics {
                        tick_duration_ns: tick_duration,
                        event_processing_latency_ns: tick_duration / events_count.max(1) as u64,
                        queue_depth: 0,               // Would need queue monitoring
                        memory_usage_bytes: 0,        // Would need memory monitoring
                        cpu_utilization_percent: 0.0, // Would need CPU monitoring
                        thread_count: 1,              // Would need thread monitoring
                    },
                )),
            })
        } else {
            None
        }
    }
}

/// Get current timestamp in nanoseconds
fn current_timestamp_ns() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
}
