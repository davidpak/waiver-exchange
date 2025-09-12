//! Configuration for SimulationClock

use crate::{DEFAULT_MAX_CONCURRENT_SYMBOLS, DEFAULT_METRICS_INTERVAL_MS, DEFAULT_TICK_CADENCE_MS};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the SimulationClock
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockConfig {
    /// Tick cadence in milliseconds (default: 1ms = 1kHz)
    pub tick_cadence_ms: u64,

    /// How to order symbols for processing
    pub symbol_ordering: SymbolOrdering,

    /// Maximum number of symbols to process concurrently
    pub max_concurrent_symbols: usize,

    /// Error recovery strategy
    pub error_recovery: ErrorRecovery,

    /// Metrics emission interval in milliseconds
    pub metrics_interval_ms: u64,

    /// Snapshot creation interval in ticks (default: 1000 ticks = 1 second at 1kHz)
    pub snapshot_interval_ticks: u64,

    /// Performance configuration
    pub performance: PerformanceConfig,

    /// Monitoring configuration
    pub monitoring: MonitoringConfig,
}

/// How symbols are ordered for processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SymbolOrdering {
    /// Deterministic ordering by symbol ID
    BySymbolId,
    /// Order by when symbol was activated
    ByActivationTime,
    /// Custom ordering (list of symbol IDs)
    Custom(Vec<u32>),
}

/// Error recovery strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorRecovery {
    /// Continue processing other symbols when one fails
    Continue,
    /// Halt entire system when any symbol fails
    Halt,
    /// Retry failed symbol N times before evicting
    Retry(usize),
}

/// Performance-related configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Enable performance profiling
    pub enable_profiling: bool,

    /// Maximum allowed tick duration in milliseconds
    pub max_tick_duration_ms: u64,

    /// Maximum allowed per-symbol processing time in milliseconds
    pub symbol_timeout_ms: u64,

    /// Enable CPU affinity for threads
    pub enable_cpu_affinity: bool,

    /// Number of threads in the processing pool
    pub thread_pool_size: usize,
}

/// Monitoring and observability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable metrics emission
    pub emit_metrics: bool,

    /// Health check interval in milliseconds
    pub health_check_interval_ms: u64,

    /// Alert on component failures
    pub alert_on_failures: bool,

    /// Enable detailed performance metrics
    pub detailed_performance_metrics: bool,

    /// Log level for the clock
    pub log_level: String,
}

impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            tick_cadence_ms: DEFAULT_TICK_CADENCE_MS,
            symbol_ordering: SymbolOrdering::BySymbolId,
            max_concurrent_symbols: DEFAULT_MAX_CONCURRENT_SYMBOLS,
            error_recovery: ErrorRecovery::Continue,
            metrics_interval_ms: DEFAULT_METRICS_INTERVAL_MS,
            snapshot_interval_ticks: 1000, // 1 second at 1kHz
            performance: PerformanceConfig::default(),
            monitoring: MonitoringConfig::default(),
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enable_profiling: false,
            max_tick_duration_ms: 10,
            symbol_timeout_ms: 5,
            enable_cpu_affinity: false,
            thread_pool_size: num_cpus::get(),
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            emit_metrics: true,
            health_check_interval_ms: 5000,
            alert_on_failures: true,
            detailed_performance_metrics: false,
            log_level: "info".to_string(),
        }
    }
}

impl ClockConfig {
    /// Get tick cadence as Duration
    pub fn tick_cadence(&self) -> Duration {
        Duration::from_millis(self.tick_cadence_ms)
    }

    /// Get metrics interval as Duration
    pub fn metrics_interval(&self) -> Duration {
        Duration::from_millis(self.metrics_interval_ms)
    }

    /// Get max tick duration as Duration
    pub fn max_tick_duration(&self) -> Duration {
        Duration::from_millis(self.performance.max_tick_duration_ms)
    }

    /// Get symbol timeout as Duration
    pub fn symbol_timeout(&self) -> Duration {
        Duration::from_millis(self.performance.symbol_timeout_ms)
    }

    /// Get health check interval as Duration
    pub fn health_check_interval(&self) -> Duration {
        Duration::from_millis(self.monitoring.health_check_interval_ms)
    }

    /// Load configuration from TOML file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: ClockConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to TOML file
    pub fn to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
