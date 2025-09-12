// Event types for ExecutionManager

// Removed unused serde imports
use std::time::Instant;
use whistle::{OrderId, Price, Qty, Side, TickId};

/// Execution ID - globally unique identifier for trades
pub type ExecutionId = u64;

/// Canonical outbound event types emitted by ExecutionManager
#[derive(Debug, Clone)]
pub enum DispatchEvent {
    /// Per-order fill event
    ExecutionReport(ExecutionReport),
    /// Market-level trade tick
    TradeEvent(TradeEvent),
    /// Order submission acknowledgment
    OrderSubmitted(OrderSubmitted),
    /// Order cancellation acknowledgment
    OrderCancelled(OrderCancelled),
    /// Order book depth updates
    BookDelta(BookDelta),
    /// End-of-tick boundary marker
    TickBoundary(TickBoundaryEvent),
    /// System diagnostics and logging
    SystemLog(SystemLog),
}

/// Per-order fill event with execution details
#[derive(Debug, Clone)]
pub struct ExecutionReport {
    /// Globally unique execution ID
    pub execution_id: ExecutionId,
    /// Order ID that was filled
    pub order_id: OrderId,
    /// Execution price
    pub price: Price,
    /// Execution quantity
    pub quantity: Qty,
    /// Side of the order (buy/sell)
    pub side: Side,
    /// Whether this order was the aggressor (taker)
    pub aggressor_flag: bool,
    /// Logical timestamp (tick)
    pub logical_timestamp: TickId,
    /// Wall-clock timestamp
    pub wall_clock_timestamp: Instant,
    /// Symbol ID
    pub symbol: u32,
}

/// Market-level trade event
#[derive(Debug, Clone)]
pub struct TradeEvent {
    /// Symbol ID
    pub symbol: u32,
    /// Execution price
    pub price: Price,
    /// Trade quantity
    pub quantity: Qty,
    /// Side of the aggressor (taker)
    pub aggressor_side: Side,
    /// Logical timestamp (tick)
    pub logical_timestamp: TickId,
    /// Wall-clock timestamp
    pub wall_clock_timestamp: Instant,
    /// Globally unique execution ID
    pub execution_id: ExecutionId,
}

/// Order submission acknowledgment
#[derive(Debug, Clone)]
pub struct OrderSubmitted {
    /// Order ID that was submitted
    pub order_id: OrderId,
    /// Logical timestamp (tick)
    pub logical_timestamp: TickId,
    /// Wall-clock timestamp
    pub wall_clock_timestamp: Instant,
    /// Symbol ID
    pub symbol: u32,
}

/// Order cancellation acknowledgment
#[derive(Debug, Clone)]
pub struct OrderCancelled {
    /// Order ID that was cancelled
    pub order_id: OrderId,
    /// Cancellation reason
    pub reason: Option<String>,
    /// Logical timestamp (tick)
    pub logical_timestamp: TickId,
    /// Wall-clock timestamp
    pub wall_clock_timestamp: Instant,
    /// Symbol ID
    pub symbol: u32,
}

/// Order book depth update
#[derive(Debug, Clone)]
pub struct BookDelta {
    /// Symbol ID
    pub symbol: u32,
    /// Price level
    pub price_level: Price,
    /// Side (buy/sell)
    pub side: Side,
    /// Quantity change (positive for increase, negative for decrease)
    pub delta: i32,
    /// New quantity at this level after the change
    pub new_quantity: Qty,
    /// Logical timestamp (tick)
    pub logical_timestamp: TickId,
    /// Wall-clock timestamp
    pub wall_clock_timestamp: Instant,
}

/// End-of-tick boundary marker
#[derive(Debug, Clone)]
pub struct TickBoundaryEvent {
    /// Tick ID that was completed
    pub tick: TickId,
    /// List of symbols that contributed to this tick
    pub flushed_symbols: Vec<u32>,
    /// Wall-clock timestamp when tick was flushed
    pub timestamp: Instant,
    /// Total events processed up to this tick
    pub events_processed: u64,
}

/// System diagnostics and logging
#[derive(Debug, Clone)]
pub struct SystemLog {
    /// Log level
    pub level: LogLevel,
    /// Log message
    pub message: String,
    /// Optional symbol context
    pub symbol: Option<u32>,
    /// Optional tick context
    pub tick: Option<TickId>,
    /// Wall-clock timestamp
    pub timestamp: Instant,
}

/// Log levels for system diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Debug information
    Debug,
    /// Informational messages
    Info,
    /// Warning messages
    Warn,
    /// Error messages
    Error,
    /// Critical errors
    Critical,
}

impl DispatchEvent {
    /// Get the symbol ID for this event
    pub fn symbol(&self) -> Option<u32> {
        match self {
            DispatchEvent::ExecutionReport(ev) => Some(ev.symbol),
            DispatchEvent::TradeEvent(ev) => Some(ev.symbol),
            DispatchEvent::OrderSubmitted(ev) => Some(ev.symbol),
            DispatchEvent::OrderCancelled(ev) => Some(ev.symbol),
            DispatchEvent::BookDelta(ev) => Some(ev.symbol),
            DispatchEvent::TickBoundary(_ev) => None, // Tick boundary applies to all symbols
            DispatchEvent::SystemLog(ev) => ev.symbol,
        }
    }

    /// Get the logical timestamp for this event
    pub fn logical_timestamp(&self) -> Option<TickId> {
        match self {
            DispatchEvent::ExecutionReport(ev) => Some(ev.logical_timestamp),
            DispatchEvent::TradeEvent(ev) => Some(ev.logical_timestamp),
            DispatchEvent::OrderSubmitted(ev) => Some(ev.logical_timestamp),
            DispatchEvent::OrderCancelled(ev) => Some(ev.logical_timestamp),
            DispatchEvent::BookDelta(ev) => Some(ev.logical_timestamp),
            DispatchEvent::TickBoundary(ev) => Some(ev.tick),
            DispatchEvent::SystemLog(ev) => ev.tick,
        }
    }

    /// Get the wall-clock timestamp for this event
    pub fn wall_clock_timestamp(&self) -> Instant {
        match self {
            DispatchEvent::ExecutionReport(ev) => ev.wall_clock_timestamp,
            DispatchEvent::TradeEvent(ev) => ev.wall_clock_timestamp,
            DispatchEvent::OrderSubmitted(ev) => ev.wall_clock_timestamp,
            DispatchEvent::OrderCancelled(ev) => ev.wall_clock_timestamp,
            DispatchEvent::BookDelta(ev) => ev.wall_clock_timestamp,
            DispatchEvent::TickBoundary(ev) => ev.timestamp,
            DispatchEvent::SystemLog(ev) => ev.timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatch_event_properties() {
        let execution_report = DispatchEvent::ExecutionReport(ExecutionReport {
            execution_id: 12345,
            order_id: 1,
            price: 150,
            quantity: 10,
            side: Side::Buy,
            aggressor_flag: true,
            logical_timestamp: 100,
            wall_clock_timestamp: Instant::now(),
            symbol: 1,
        });

        assert_eq!(execution_report.symbol(), Some(1));
        assert_eq!(execution_report.logical_timestamp(), Some(100));
    }

    #[test]
    fn test_tick_boundary_event() {
        let tick_boundary = DispatchEvent::TickBoundary(TickBoundaryEvent {
            tick: 100,
            flushed_symbols: vec![1, 2, 3],
            timestamp: Instant::now(),
            events_processed: 1000,
        });

        assert_eq!(tick_boundary.symbol(), None);
        assert_eq!(tick_boundary.logical_timestamp(), Some(100));
    }

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
        assert!(LogLevel::Error < LogLevel::Critical);
    }
}
