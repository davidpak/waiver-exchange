// Configuration structures for ExecutionManager

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Main configuration for ExecutionManager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecManagerConfig {
    /// Maximum number of events to process in a single batch
    pub batch_size: usize,

    /// Configuration for execution ID allocation
    pub execution_id_config: ExecutionIdConfig,

    /// Configuration for event normalization
    pub normalization_config: NormalizationConfig,

    /// Configuration for event fanout
    pub fanout_config: FanoutConfig,

    /// Configuration for tick tracking
    pub tick_tracking_config: TickTrackingConfig,

    /// Configuration for shutdown behavior
    pub shutdown_config: ShutdownConfig,
}

impl Default for ExecManagerConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            execution_id_config: Default::default(),
            normalization_config: Default::default(),
            fanout_config: Default::default(),
            tick_tracking_config: Default::default(),
            shutdown_config: Default::default(),
        }
    }
}

/// Configuration for execution ID allocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionIdConfig {
    /// Mode for execution ID generation
    pub mode: ExecutionIdMode,

    /// Shard ID for distributed execution ID generation
    pub shard_id: Option<u32>,

    /// Number of bits to shift tick ID in execution ID
    pub tick_shift_bits: u8,
}

impl Default for ExecutionIdConfig {
    fn default() -> Self {
        Self { mode: ExecutionIdMode::Monotonic, shard_id: None, tick_shift_bits: 12 }
    }
}

/// Execution ID generation mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionIdMode {
    /// Monotonic counter (single instance)
    Monotonic,
    /// Sharded counter (distributed instances)
    Sharded,
}

/// Configuration for event normalization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationConfig {
    /// Whether to add wall-clock timestamps to events
    pub add_wall_clock_timestamps: bool,

    /// Whether to validate event consistency
    pub validate_events: bool,

    /// Whether to add derived fields (aggressor side, liquidity flags)
    pub add_derived_fields: bool,
}

impl Default for NormalizationConfig {
    fn default() -> Self {
        Self { add_wall_clock_timestamps: true, validate_events: true, add_derived_fields: true }
    }
}

/// Configuration for event fanout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanoutConfig {
    /// Configuration for each downstream destination
    pub destinations: Vec<FanoutDestinationConfig>,

    /// Default backpressure policy
    pub default_backpressure: BackpressureConfig,
}

impl Default for FanoutConfig {
    fn default() -> Self {
        Self {
            destinations: vec![
                FanoutDestinationConfig {
                    name: "replay".to_string(),
                    enabled: true,
                    backpressure: BackpressureConfig::Fatal,
                    queue_capacity: 8192,
                },
                FanoutDestinationConfig {
                    name: "analytics".to_string(),
                    enabled: true,
                    backpressure: BackpressureConfig::Drop,
                    queue_capacity: 4096,
                },
                FanoutDestinationConfig {
                    name: "webui".to_string(),
                    enabled: true,
                    backpressure: BackpressureConfig::Drop,
                    queue_capacity: 2048,
                },
            ],
            default_backpressure: BackpressureConfig::Fatal,
        }
    }
}

/// Configuration for a specific fanout destination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanoutDestinationConfig {
    /// Name of the destination
    pub name: String,

    /// Whether this destination is enabled
    pub enabled: bool,

    /// Backpressure policy for this destination
    pub backpressure: BackpressureConfig,

    /// Queue capacity for this destination
    pub queue_capacity: usize,
}

/// Backpressure handling configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackpressureConfig {
    /// System exits on overflow (recommended for data integrity)
    Fatal,
    /// Drop events on overflow (with metrics tracking)
    Drop,
    /// Block until space is available (not recommended for hot path)
    Block,
}

/// Configuration for tick tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickTrackingConfig {
    /// Maximum time to wait for all symbols to complete a tick
    pub tick_timeout: Duration,

    /// Whether to emit warnings for slow symbols
    pub warn_on_slow_symbols: bool,

    /// Threshold for considering a symbol slow (percentage of timeout)
    pub slow_symbol_threshold: f64,
}

impl Default for TickTrackingConfig {
    fn default() -> Self {
        Self {
            tick_timeout: Duration::from_micros(100), // 100μs timeout
            warn_on_slow_symbols: true,
            slow_symbol_threshold: 0.8, // 80% of timeout
        }
    }
}

/// Configuration for shutdown behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownConfig {
    /// Maximum time to wait for graceful shutdown
    pub shutdown_timeout: Duration,

    /// Whether to flush remaining events during shutdown
    pub flush_on_shutdown: bool,

    /// Whether to wait for all downstream systems to acknowledge shutdown
    pub wait_for_downstream: bool,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            shutdown_timeout: Duration::from_secs(5),
            flush_on_shutdown: true,
            wait_for_downstream: false,
        }
    }
}
