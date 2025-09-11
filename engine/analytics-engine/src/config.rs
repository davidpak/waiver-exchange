//! # Configuration Management
//!
//! Configuration structures and management for the AnalyticsEngine.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
// Removed unused import

/// Main configuration for the AnalyticsEngine
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalyticsConfig {
    /// Storage configuration
    pub storage: StorageConfig,
    /// Ingestion configuration
    pub ingestion: IngestionConfig,
    /// Aggregation configuration
    pub aggregation: AggregationConfig,
    /// Query configuration
    pub query: QueryConfig,
    /// Retention configuration
    pub retention: RetentionConfig,
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Base path for parquet files
    pub base_path: PathBuf,
    /// Partition strategy (hourly, daily)
    pub partition_strategy: PartitionStrategy,
    /// Compression algorithm
    pub compression: CompressionType,
    /// Maximum file size before rotation
    pub max_file_size_mb: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from("./analytics_data"),
            partition_strategy: PartitionStrategy::Hourly,
            compression: CompressionType::Snappy,
            max_file_size_mb: 100,
        }
    }
}

/// Ingestion configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionConfig {
    /// Sampling interval (seconds)
    pub sampling_interval_secs: u64,
    /// Sampling interval (ticks)
    pub sampling_interval_ticks: u64,
    /// Maximum batch size for processing
    pub max_batch_size: usize,
    /// Buffer size for incoming events
    pub buffer_size: usize,
    /// Timeout for batch processing
    pub batch_timeout_ms: u64,
}

impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            sampling_interval_secs: 1,
            sampling_interval_ticks: 100,
            max_batch_size: 1000,
            buffer_size: 10000,
            batch_timeout_ms: 100,
        }
    }
}

/// Aggregation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationConfig {
    /// Aggregation window sizes (seconds)
    pub window_sizes: Vec<u64>,
    /// Enable real-time aggregation
    pub real_time_enabled: bool,
    /// Maximum memory usage for aggregation
    pub max_memory_mb: u64,
}

impl Default for AggregationConfig {
    fn default() -> Self {
        Self {
            window_sizes: vec![1, 60, 3600, 86400], // 1s, 1m, 1h, 1d
            real_time_enabled: true,
            max_memory_mb: 512,
        }
    }
}

/// Query configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryConfig {
    /// Maximum query timeout (seconds)
    pub max_timeout_secs: u64,
    /// Maximum result size
    pub max_result_size: usize,
    /// Enable query caching
    pub cache_enabled: bool,
    /// Cache size (number of queries)
    pub cache_size: usize,
}

impl Default for QueryConfig {
    fn default() -> Self {
        Self {
            max_timeout_secs: 30,
            max_result_size: 100000,
            cache_enabled: true,
            cache_size: 1000,
        }
    }
}

/// Retention configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// Hot data retention (days)
    pub hot_retention_days: u32,
    /// Warm data retention (days)
    pub warm_retention_days: u32,
    /// Cold data retention (days)
    pub cold_retention_days: u32,
    /// Cleanup interval (hours)
    pub cleanup_interval_hours: u32,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            hot_retention_days: 7,
            warm_retention_days: 30,
            cold_retention_days: 90,
            cleanup_interval_hours: 24,
        }
    }
}

/// Partition strategy for data storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PartitionStrategy {
    /// Partition by hour
    Hourly,
    /// Partition by day
    Daily,
    /// Partition by month
    Monthly,
}

/// Compression algorithm for parquet files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionType {
    /// No compression
    None,
    /// Snappy compression (fast, good ratio)
    Snappy,
    /// Gzip compression (slower, better ratio)
    Gzip,
    /// LZ4 compression (very fast, moderate ratio)
    Lz4,
}

impl AnalyticsConfig {
    /// Load configuration from file
    pub fn load_from_file(path: &str) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let config: AnalyticsConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn save_to_file(&self, path: &str) -> Result<(), anyhow::Error> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get storage path for the config
    pub fn storage_path(&self) -> &PathBuf {
        &self.storage.base_path
    }
}
