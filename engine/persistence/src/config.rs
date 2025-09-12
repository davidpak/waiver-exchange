//! Configuration for the persistence layer

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Configuration for the persistence layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    /// Base directory for persistence files
    pub data_dir: PathBuf,

    /// WAL configuration
    pub wal: WalConfig,

    /// Snapshot configuration
    pub snapshot: SnapshotConfig,

    /// Retention policy
    pub retention: RetentionConfig,
}

/// Write-Ahead Log configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalConfig {
    /// Maximum size of a single WAL file before rotation
    pub max_file_size: u64,

    /// Maximum number of WAL files to keep
    pub max_files: usize,

    /// Whether to compress WAL files
    pub compress: bool,

    /// Flush interval for WAL entries
    pub flush_interval: Duration,

    /// Whether to fsync on every write (for maximum durability)
    pub fsync_every_write: bool,
}

/// Snapshot configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotConfig {
    /// Interval between automatic snapshots
    pub interval: Duration,

    /// Maximum number of snapshots to keep
    pub max_snapshots: usize,

    /// Whether to compress snapshots
    pub compress: bool,

    /// Whether to create snapshots on shutdown
    pub snapshot_on_shutdown: bool,
}

/// Data retention configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// How long to keep WAL files
    pub wal_retention: Duration,

    /// How long to keep snapshots
    pub snapshot_retention: Duration,

    /// Whether to enable automatic cleanup
    pub auto_cleanup: bool,

    /// Cleanup interval
    pub cleanup_interval: Duration,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
            wal: WalConfig::default(),
            snapshot: SnapshotConfig::default(),
            retention: RetentionConfig::default(),
        }
    }
}

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            max_file_size: 100 * 1024 * 1024, // 100MB
            max_files: 10,
            compress: true,
            flush_interval: Duration::from_millis(100),
            fsync_every_write: false, // Balance between performance and durability
        }
    }
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(300), // 5 minutes
            max_snapshots: 24,                  // Keep 24 snapshots (2 hours at 5min intervals)
            compress: true,
            snapshot_on_shutdown: true,
        }
    }
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            wal_retention: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            snapshot_retention: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
            auto_cleanup: true,
            cleanup_interval: Duration::from_secs(60 * 60), // 1 hour
        }
    }
}

impl PersistenceConfig {
    /// Create a new configuration with custom data directory
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self { data_dir: data_dir.into(), ..Default::default() }
    }

    /// Get the WAL directory path
    pub fn wal_dir(&self) -> PathBuf {
        self.data_dir.join("wal")
    }

    /// Get the snapshots directory path
    pub fn snapshots_dir(&self) -> PathBuf {
        self.data_dir.join("snapshots")
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.wal.max_file_size == 0 {
            return Err("WAL max_file_size must be greater than 0".to_string());
        }

        if self.wal.max_files == 0 {
            return Err("WAL max_files must be greater than 0".to_string());
        }

        if self.snapshot.max_snapshots == 0 {
            return Err("Snapshot max_snapshots must be greater than 0".to_string());
        }

        Ok(())
    }
}
