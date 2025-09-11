//! # Analytics Protobuf Definitions
//!
//! Manual protobuf definitions for analytics events.

use serde::{Deserialize, Serialize};

/// Main analytics event from ExecutionManager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsEvent {
    pub timestamp_ns: u64,
    pub tick_id: u64,
    pub symbol: String,
    pub event_type: i32,
    pub data: Option<analytics_event::Data>,
}

pub mod analytics_event {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Data {
        Performance(PerformanceMetrics),
        Business(BusinessMetrics),
        Health(SystemHealthMetrics),
        Operational(OperationalMetrics),
    }
}

/// Event type enumeration
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EventType {
    Performance = 0,
    Business = 1,
    SystemHealth = 2,
    Operational = 3,
}

/// Performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub tick_duration_ns: u64,
    pub event_processing_latency_ns: u64,
    pub queue_depth: u32,
    pub memory_usage_bytes: u64,
    pub cpu_utilization_percent: f64,
    pub thread_count: u32,
}

/// Business metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessMetrics {
    pub orders_processed: u32,
    pub trades_executed: u32,
    pub volume_traded: u64,
    pub active_accounts: u32,
    pub order_book_depth: u32,
    pub average_trade_size: f64,
}

/// System health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthMetrics {
    pub engine_crashed: bool,
    pub queue_overflows: u32,
    pub memory_allocation_failures: u32,
    pub error_rate_percent: f64,
    pub uptime_seconds: u64,
    pub error_message: String,
}

/// Operational metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationalMetrics {
    pub symbol_activated: bool,
    pub symbol_evicted: bool,
    pub thread_utilization_percent: f64,
    pub network_bytes_sent: u64,
    pub disk_bytes_written: u64,
    pub active_symbols: u32,
}

impl AnalyticsEvent {
    /// Get event type as enum
    pub fn event_type(&self) -> EventType {
        match self.event_type {
            0 => EventType::Performance,
            1 => EventType::Business,
            2 => EventType::SystemHealth,
            3 => EventType::Operational,
            _ => EventType::Performance, // Default fallback
        }
    }
}
