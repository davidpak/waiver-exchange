//! Snapshot implementation for periodic state captures
//!
//! Snapshots provide periodic "photos" of the entire system state, enabling
//! faster recovery by starting from a known good state and replaying only
//! recent WAL entries.

use crate::config::SnapshotConfig;
use crate::error::{PersistenceError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// A snapshot of the system state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Unique identifier for this snapshot
    pub id: Uuid,

    /// Timestamp when the snapshot was created
    pub timestamp: DateTime<Utc>,

    /// The tick at which this snapshot was taken
    pub tick: u64,

    /// The system state at this point in time
    pub state: SystemState,

    /// Metadata about the snapshot
    pub metadata: SnapshotMetadata,
}

/// System state captured in a snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemState {
    /// Order books for each symbol
    pub order_books: HashMap<u32, OrderBookState>,

    /// Account balances and positions
    pub accounts: HashMap<u32, AccountState>,

    /// Active symbols in the system
    pub active_symbols: Vec<u32>,

    /// System configuration
    pub config: SystemConfig,

    /// Statistics
    pub stats: SystemStats,
}

/// Order book state for a single symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookState {
    /// Symbol ID
    pub symbol_id: u32,

    /// Buy orders (price -> quantity)
    pub buy_orders: HashMap<u64, u64>,

    /// Sell orders (price -> quantity)
    pub sell_orders: HashMap<u64, u64>,

    /// Last trade price
    pub last_trade_price: Option<u64>,

    /// Last trade quantity
    pub last_trade_quantity: Option<u64>,

    /// Last trade timestamp
    pub last_trade_timestamp: Option<DateTime<Utc>>,
}

/// Account state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    /// Account ID
    pub account_id: u32,

    /// Available balance
    pub balance: u64,

    /// Positions per symbol
    pub positions: HashMap<u32, i64>, // positive = long, negative = short

    /// Open orders
    pub open_orders: Vec<u64>, // order IDs
}

/// System configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    /// Maximum number of symbols
    pub max_symbols: u32,

    /// Maximum number of accounts
    pub max_accounts: u32,

    /// Tick duration in nanoseconds
    pub tick_duration_ns: u64,
}

/// System statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    /// Total number of orders processed
    pub total_orders: u64,

    /// Total number of trades executed
    pub total_trades: u64,

    /// Total volume traded
    pub total_volume: u64,

    /// Current tick
    pub current_tick: u64,

    /// System uptime
    pub uptime_seconds: u64,
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Snapshot version
    pub version: String,

    /// Compression used
    pub compression: Option<String>,

    /// File size
    pub file_size: u64,

    /// Creation duration
    pub creation_duration_ms: u64,
}

/// Snapshot file information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotFileInfo {
    /// File path
    pub path: PathBuf,

    /// File size in bytes
    pub size: u64,

    /// Snapshot ID
    pub snapshot_id: Uuid,

    /// Tick when snapshot was taken
    pub tick: u64,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Snapshot manager
pub struct SnapshotManager {
    config: SnapshotConfig,
    snapshots_dir: PathBuf,
    current_snapshot: Arc<Mutex<Option<Snapshot>>>,
}

impl SnapshotManager {
    /// Create a new snapshot manager
    pub fn new(config: SnapshotConfig, snapshots_dir: PathBuf) -> Result<Self> {
        // Ensure snapshots directory exists
        std::fs::create_dir_all(&snapshots_dir).map_err(PersistenceError::Io)?;

        Ok(Self { config, snapshots_dir, current_snapshot: Arc::new(Mutex::new(None)) })
    }

    /// Create a new snapshot
    pub async fn create_snapshot(&self, state: SystemState, tick: u64) -> Result<Uuid> {
        let start_time = std::time::Instant::now();

        let snapshot = Snapshot {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            tick,
            state,
            metadata: SnapshotMetadata {
                version: "1.0".to_string(),
                compression: if self.config.compress { Some("gzip".to_string()) } else { None },
                file_size: 0,            // Will be set after writing
                creation_duration_ms: 0, // Will be set after writing
            },
        };

        // Write snapshot to file
        let file_path = self.write_snapshot_file(&snapshot).await?;

        // Update metadata with actual file size
        let file_size = std::fs::metadata(&file_path).map_err(PersistenceError::Io)?.len();

        let creation_duration = start_time.elapsed();

        // Update snapshot with final metadata
        let mut final_snapshot = snapshot;
        final_snapshot.metadata.file_size = file_size;
        final_snapshot.metadata.creation_duration_ms = creation_duration.as_millis() as u64;

        // Store current snapshot
        {
            let mut current = self.current_snapshot.lock().await;
            *current = Some(final_snapshot.clone());
        }

        tracing::info!(
            "Created snapshot {} for tick {} ({}ms, {} bytes)",
            final_snapshot.id,
            tick,
            final_snapshot.metadata.creation_duration_ms,
            final_snapshot.metadata.file_size
        );

        // Clean up old snapshots AFTER creating new one
        self.cleanup_old_snapshots().await?;

        Ok(final_snapshot.id)
    }

    /// Load the most recent snapshot
    pub async fn load_latest_snapshot(&self) -> Result<Option<Snapshot>> {
        let snapshots = self.list_snapshots().await?;

        if snapshots.is_empty() {
            tracing::info!("No snapshots found, starting with clean state");
            return Ok(None);
        }

        // Debug: Log all available snapshots
        tracing::info!("Found {} snapshots:", snapshots.len());
        for (i, snapshot) in snapshots.iter().enumerate() {
            tracing::info!(
                "  Snapshot {}: ID={}, tick={}, path={:?}",
                i + 1,
                snapshot.snapshot_id,
                snapshot.tick,
                snapshot.path
            );
        }

        // Find the most recent snapshot
        let latest = snapshots
            .iter()
            .max_by_key(|s| s.tick)
            .ok_or_else(|| PersistenceError::not_found("No snapshots found"))?;

        tracing::info!(
            "Loading latest snapshot: ID={}, tick={}, path={:?}",
            latest.snapshot_id,
            latest.tick,
            latest.path
        );

        let snapshot = self.load_snapshot(&latest.path).await?;

        tracing::info!(
            "Successfully loaded snapshot: ID={}, tick={}, symbols={}, order_books={}",
            snapshot.id,
            snapshot.tick,
            snapshot.state.active_symbols.len(),
            snapshot.state.order_books.len()
        );

        Ok(Some(snapshot))
    }

    /// Load a specific snapshot by ID
    pub async fn load_snapshot_by_id(&self, snapshot_id: Uuid) -> Result<Option<Snapshot>> {
        let snapshots = self.list_snapshots().await?;

        for snapshot_info in snapshots {
            if snapshot_info.snapshot_id == snapshot_id {
                return Ok(Some(self.load_snapshot(&snapshot_info.path).await?));
            }
        }

        Ok(None)
    }

    /// Get information about all snapshots
    pub async fn list_snapshots(&self) -> Result<Vec<SnapshotFileInfo>> {
        let mut snapshots = Vec::new();

        let entries = std::fs::read_dir(&self.snapshots_dir).map_err(PersistenceError::Io)?;

        for entry in entries {
            let entry = entry.map_err(PersistenceError::Io)?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("snapshot") {
                let metadata = entry.metadata().map_err(PersistenceError::Io)?;
                let size = metadata.len();

                // Try to read snapshot metadata
                if let Ok(snapshot) = self.load_snapshot(&path).await {
                    snapshots.push(SnapshotFileInfo {
                        path,
                        size,
                        snapshot_id: snapshot.id,
                        tick: snapshot.tick,
                        created_at: snapshot.timestamp,
                    });
                }
            }
        }

        // Sort by tick (oldest first) so we can remove the oldest ones
        snapshots.sort_by_key(|s| s.tick);

        Ok(snapshots)
    }

    /// Clean up old snapshots based on retention policy
    pub async fn cleanup_old_snapshots(&self) -> Result<()> {
        let snapshots = self.list_snapshots().await?;

        tracing::info!(
            "Cleanup: Found {} snapshots, max allowed: {}",
            snapshots.len(),
            self.config.max_snapshots
        );

        if snapshots.len() <= self.config.max_snapshots {
            tracing::info!("Cleanup: No cleanup needed, within limit");
            return Ok(());
        }

        let snapshots_to_remove = snapshots.len() - self.config.max_snapshots;
        tracing::info!("Cleanup: Need to remove {} snapshots", snapshots_to_remove);

        // Log the snapshots we're about to remove (the oldest ones)
        for (i, snapshot) in snapshots.iter().take(snapshots_to_remove).enumerate() {
            tracing::info!(
                "Cleanup: Will remove snapshot {}: tick={}, path={:?}",
                i + 1,
                snapshot.tick,
                snapshot.path
            );
        }

        // Remove the oldest snapshots (first ones in the sorted list)
        for snapshot in snapshots.iter().take(snapshots_to_remove) {
            std::fs::remove_file(&snapshot.path).map_err(PersistenceError::Io)?;

            tracing::info!("Removed old snapshot: {:?}", snapshot.path);
        }

        Ok(())
    }

    // Private methods

    async fn write_snapshot_file(&self, snapshot: &Snapshot) -> Result<PathBuf> {
        let filename = format!("snapshot_{}_{:016x}.snapshot", snapshot.id, snapshot.tick);
        let file_path = self.snapshots_dir.join(filename);

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&file_path)
            .map_err(PersistenceError::Io)?;

        let mut writer = BufWriter::new(file);

        if self.config.compress {
            // TODO: Implement compression
            // For now, write uncompressed
            serde_json::to_writer(&mut writer, snapshot)
                .map_err(PersistenceError::Serialization)?;
        } else {
            serde_json::to_writer(&mut writer, snapshot)
                .map_err(PersistenceError::Serialization)?;
        }

        writer.flush().map_err(PersistenceError::Io)?;

        Ok(file_path)
    }

    async fn load_snapshot(&self, path: &Path) -> Result<Snapshot> {
        let file = File::open(path).map_err(PersistenceError::Io)?;

        let reader = BufReader::new(file);

        // TODO: Handle compression
        let snapshot: Snapshot =
            serde_json::from_reader(reader).map_err(PersistenceError::Serialization)?;

        Ok(snapshot)
    }
}
