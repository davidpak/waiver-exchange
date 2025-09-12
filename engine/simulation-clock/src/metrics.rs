//! Metrics collection for SimulationClock

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use symbol_coordinator::SymbolId;

/// Metrics collected by the SimulationClock
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockMetrics {
    /// Current tick number
    pub current_tick: u64,

    /// Duration of the last tick in nanoseconds
    pub tick_duration_ns: u64,

    /// Current tick rate in Hz
    pub tick_rate_hz: f64,

    /// Number of active symbols
    pub active_symbols: u32,

    /// Number of symbols processed in last tick
    pub symbols_processed: u32,

    /// Number of symbol failures in last tick
    pub symbol_failures: u32,

    /// Average tick duration in nanoseconds
    pub avg_tick_duration_ns: u64,

    /// Maximum tick duration in nanoseconds
    pub max_tick_duration_ns: u64,

    /// 95th percentile tick duration in nanoseconds
    pub p95_tick_duration_ns: u64,

    /// 99th percentile tick duration in nanoseconds
    pub p99_tick_duration_ns: u64,

    /// System uptime in seconds
    pub system_uptime_seconds: u64,

    /// Total ticks processed
    pub total_ticks_processed: u64,

    /// Total events processed
    pub total_events_processed: u64,

    /// Total symbol failures
    pub total_symbol_failures: u64,
}

/// Per-symbol metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMetrics {
    /// Symbol ID
    pub symbol_id: SymbolId,

    /// Processing time for last tick in nanoseconds
    pub processing_time_ns: u64,

    /// Average processing time in nanoseconds
    pub avg_processing_time_ns: u64,

    /// Number of events generated in last tick
    pub events_generated: u32,

    /// Total events generated
    pub total_events_generated: u64,

    /// Number of failures
    pub failure_count: u32,

    /// Last failure timestamp
    pub last_failure_time: Option<u64>,
}

/// Metrics collector for the SimulationClock
pub struct MetricsCollector {
    // Tick metrics
    current_tick: AtomicU64,
    tick_durations: Arc<Vec<AtomicU64>>,
    max_tick_duration: AtomicU64,
    total_ticks: AtomicU64,

    // Symbol metrics
    active_symbols: AtomicU64,
    symbols_processed: AtomicU64,
    symbol_failures: AtomicU64,
    total_symbol_failures: AtomicU64,

    // Event metrics
    total_events: AtomicU64,

    // Timing
    start_time: Instant,

    // Configuration
    history_size: usize,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(history_size: usize) -> Self {
        let mut durations = Vec::with_capacity(history_size);
        for _ in 0..history_size {
            durations.push(AtomicU64::new(0));
        }

        Self {
            current_tick: AtomicU64::new(0),
            tick_durations: Arc::new(durations),
            max_tick_duration: AtomicU64::new(0),
            total_ticks: AtomicU64::new(0),
            active_symbols: AtomicU64::new(0),
            symbols_processed: AtomicU64::new(0),
            symbol_failures: AtomicU64::new(0),
            total_symbol_failures: AtomicU64::new(0),
            total_events: AtomicU64::new(0),
            start_time: Instant::now(),
            history_size,
        }
    }

    /// Record a completed tick
    pub fn record_tick(
        &self,
        tick: u64,
        duration: Duration,
        symbols_processed: u32,
        symbol_failures: u32,
    ) {
        let duration_ns = duration.as_nanos() as u64;
        let index = (tick as usize) % self.history_size;

        self.current_tick.store(tick, Ordering::Relaxed);
        self.tick_durations[index].store(duration_ns, Ordering::Relaxed);
        self.total_ticks.fetch_add(1, Ordering::Relaxed);
        self.symbols_processed.store(symbols_processed as u64, Ordering::Relaxed);
        self.symbol_failures.store(symbol_failures as u64, Ordering::Relaxed);

        // Update max duration
        let mut max_duration = self.max_tick_duration.load(Ordering::Relaxed);
        while duration_ns > max_duration {
            match self.max_tick_duration.compare_exchange_weak(
                max_duration,
                duration_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => max_duration = current,
            }
        }
    }

    /// Record symbol processing
    pub fn record_symbol_processing(&self, _symbol_id: SymbolId, _processing_time: Duration) {
        // For now, we'll track this at the tick level
        // In the future, we can add per-symbol tracking
    }

    /// Record symbol failure
    pub fn record_symbol_failure(&self, _symbol_id: SymbolId) {
        self.total_symbol_failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Record events processed
    pub fn record_events_processed(&self, event_count: u32) {
        self.total_events.fetch_add(event_count as u64, Ordering::Relaxed);
    }

    /// Update active symbol count
    pub fn update_active_symbols(&self, count: u32) {
        self.active_symbols.store(count as u64, Ordering::Relaxed);
    }

    /// Get current metrics
    pub fn get_metrics(&self) -> ClockMetrics {
        let current_tick = self.current_tick.load(Ordering::Relaxed);
        let uptime = self.start_time.elapsed().as_secs();

        // Calculate tick rate (ticks per second)
        let tick_rate_hz = if uptime > 0 {
            self.total_ticks.load(Ordering::Relaxed) as f64 / uptime as f64
        } else {
            0.0
        };

        // Get last tick duration
        let last_index = (current_tick as usize) % self.history_size;
        let tick_duration_ns = self.tick_durations[last_index].load(Ordering::Relaxed);

        // Calculate statistics from recent history
        let mut durations: Vec<u64> = self
            .tick_durations
            .iter()
            .map(|d| d.load(Ordering::Relaxed))
            .filter(|&d| d > 0)
            .collect();

        durations.sort_unstable();

        let avg_tick_duration_ns = if !durations.is_empty() {
            durations.iter().sum::<u64>() / durations.len() as u64
        } else {
            0
        };

        let p95_tick_duration_ns = if !durations.is_empty() {
            let index = (durations.len() as f64 * 0.95) as usize;
            durations[index.min(durations.len() - 1)]
        } else {
            0
        };

        let p99_tick_duration_ns = if !durations.is_empty() {
            let index = (durations.len() as f64 * 0.99) as usize;
            durations[index.min(durations.len() - 1)]
        } else {
            0
        };

        ClockMetrics {
            current_tick,
            tick_duration_ns,
            tick_rate_hz,
            active_symbols: self.active_symbols.load(Ordering::Relaxed) as u32,
            symbols_processed: self.symbols_processed.load(Ordering::Relaxed) as u32,
            symbol_failures: self.symbol_failures.load(Ordering::Relaxed) as u32,
            avg_tick_duration_ns,
            max_tick_duration_ns: self.max_tick_duration.load(Ordering::Relaxed),
            p95_tick_duration_ns,
            p99_tick_duration_ns,
            system_uptime_seconds: uptime,
            total_ticks_processed: self.total_ticks.load(Ordering::Relaxed),
            total_events_processed: self.total_events.load(Ordering::Relaxed),
            total_symbol_failures: self.total_symbol_failures.load(Ordering::Relaxed),
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.current_tick.store(0, Ordering::Relaxed);
        self.max_tick_duration.store(0, Ordering::Relaxed);
        self.total_ticks.store(0, Ordering::Relaxed);
        self.active_symbols.store(0, Ordering::Relaxed);
        self.symbols_processed.store(0, Ordering::Relaxed);
        self.symbol_failures.store(0, Ordering::Relaxed);
        self.total_symbol_failures.store(0, Ordering::Relaxed);
        self.total_events.store(0, Ordering::Relaxed);

        for duration in self.tick_durations.iter() {
            duration.store(0, Ordering::Relaxed);
        }
    }
}
