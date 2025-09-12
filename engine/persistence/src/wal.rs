//! Write-Ahead Log (WAL) implementation
//!
//! The WAL is a transaction log where every change to the order book is written
//! before being applied in memory. This ensures durability and enables recovery.

use crate::config::WalConfig;
use crate::error::{PersistenceError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Instant;
use uuid::Uuid;

/// A single entry in the Write-Ahead Log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    /// Unique identifier for this entry
    pub id: Uuid,

    /// Timestamp when the entry was created
    pub timestamp: DateTime<Utc>,

    /// The operation type
    pub operation: WalOperation,

    /// Sequence number for ordering
    pub sequence: u64,

    /// Checksum for integrity verification
    pub checksum: u32,
}

/// Types of operations that can be logged
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalOperation {
    /// Order submission
    SubmitOrder {
        symbol_id: u32,
        account_id: u32,
        side: String,       // "buy" or "sell"
        order_type: String, // "limit" or "market"
        price: Option<u64>,
        quantity: u64,
        order_id: u64,
    },

    /// Order cancellation
    CancelOrder { symbol_id: u32, order_id: u64, account_id: u32 },

    /// Order modification
    ModifyOrder {
        symbol_id: u32,
        order_id: u64,
        account_id: u32,
        new_price: Option<u64>,
        new_quantity: u64,
    },

    /// Trade execution
    Trade {
        symbol_id: u32,
        buy_order_id: u64,
        sell_order_id: u64,
        price: u64,
        quantity: u64,
        timestamp: DateTime<Utc>,
    },

    /// System checkpoint
    Checkpoint { tick: u64, timestamp: DateTime<Utc> },
}

/// WAL file metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalFileInfo {
    /// File path
    pub path: PathBuf,

    /// File size in bytes
    pub size: u64,

    /// First sequence number in this file
    pub first_sequence: u64,

    /// Last sequence number in this file
    pub last_sequence: u64,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Write-Ahead Log implementation
pub struct Wal {
    config: WalConfig,
    wal_dir: PathBuf,
    current_file: Arc<Mutex<Option<WalFile>>>,
    sequence_counter: Arc<Mutex<u64>>,
    last_flush: Arc<Mutex<Instant>>,
}

/// Current WAL file being written to
struct WalFile {
    #[allow(dead_code)]
    path: PathBuf,
    writer: BufWriter<File>,
    current_size: u64,
    #[allow(dead_code)]
    first_sequence: u64,
    last_sequence: u64,
}

impl Wal {
    /// Create a new WAL instance
    pub fn new(config: WalConfig, wal_dir: PathBuf) -> Result<Self> {
        // Ensure WAL directory exists
        std::fs::create_dir_all(&wal_dir).map_err(PersistenceError::Io)?;

        // Find the highest sequence number from existing files
        let sequence_counter = Self::find_highest_sequence(&wal_dir)?;

        Ok(Self {
            config,
            wal_dir,
            current_file: Arc::new(Mutex::new(None)),
            sequence_counter: Arc::new(Mutex::new(sequence_counter)),
            last_flush: Arc::new(Mutex::new(Instant::now())),
        })
    }

    /// Write an entry to the WAL
    pub async fn write_entry(&self, operation: WalOperation) -> Result<u64> {
        let sequence = {
            let mut counter = self.sequence_counter.lock().await;
            *counter += 1;
            *counter
        };

        let entry = WalEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation,
            sequence,
            checksum: 0, // TODO: Calculate checksum
        };

        // Write to current file
        self.write_to_file(&entry).await?;

        // Check if we need to rotate the file
        self.check_rotation().await?;

        // Check if we need to flush
        self.check_flush().await?;

        Ok(sequence)
    }

    /// Flush all pending writes to disk
    pub async fn flush(&self) -> Result<()> {
        let mut current_file = self.current_file.lock().await;
        if let Some(ref mut file) = *current_file {
            file.writer.flush().map_err(PersistenceError::Io)?;

            if self.config.fsync_every_write {
                file.writer.get_ref().sync_all().map_err(PersistenceError::Io)?;
            }
        }

        let mut last_flush = self.last_flush.lock().await;
        *last_flush = Instant::now();

        Ok(())
    }

    /// Get information about all WAL files
    pub async fn list_files(&self) -> Result<Vec<WalFileInfo>> {
        let mut files = Vec::new();

        let entries = std::fs::read_dir(&self.wal_dir).map_err(PersistenceError::Io)?;

        for entry in entries {
            let entry = entry.map_err(PersistenceError::Io)?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("wal") {
                let metadata = entry.metadata().map_err(PersistenceError::Io)?;
                let size = metadata.len();

                // Read first and last sequence from file
                let (first_sequence, last_sequence) = Wal::read_sequence_range(&path)?;

                files.push(WalFileInfo {
                    path,
                    size,
                    first_sequence,
                    last_sequence,
                    created_at: metadata.created().map_err(PersistenceError::Io)?.into(),
                });
            }
        }

        // Sort by sequence number
        files.sort_by_key(|f| f.first_sequence);

        Ok(files)
    }

    /// Clean up old WAL files based on retention policy
    pub async fn cleanup_old_files(&self, max_files: usize) -> Result<()> {
        let files = self.list_files().await?;

        if files.len() <= max_files {
            return Ok(());
        }

        let files_to_remove = files.len() - max_files;
        for file in files.iter().take(files_to_remove) {
            std::fs::remove_file(&file.path).map_err(PersistenceError::Io)?;

            tracing::info!("Removed old WAL file: {:?}", file.path);
        }

        Ok(())
    }

    // Private methods

    async fn write_to_file(&self, entry: &WalEntry) -> Result<()> {
        let mut current_file = self.current_file.lock().await;

        // Open new file if needed
        if current_file.is_none() {
            *current_file = Some(self.create_new_file().await?);
        }

        let file = current_file.as_mut().unwrap();

        // Serialize entry to JSON
        let json = serde_json::to_string(entry).map_err(PersistenceError::Serialization)?;

        // Write entry (one per line)
        writeln!(file.writer, "{json}").map_err(PersistenceError::Io)?;

        // Update file metadata
        file.current_size += json.len() as u64 + 1; // +1 for newline
        file.last_sequence = entry.sequence;

        Ok(())
    }

    async fn create_new_file(&self) -> Result<WalFile> {
        let sequence = {
            let counter = self.sequence_counter.lock().await;
            *counter
        };

        let filename = format!("wal_{sequence:016x}.wal");
        let path = self.wal_dir.join(filename);

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(PersistenceError::Io)?;

        let writer = BufWriter::new(file);

        Ok(WalFile {
            path,
            writer,
            current_size: 0,
            first_sequence: sequence,
            last_sequence: sequence,
        })
    }

    async fn check_rotation(&self) -> Result<()> {
        let mut current_file = self.current_file.lock().await;

        if let Some(ref mut file) = *current_file {
            if file.current_size >= self.config.max_file_size {
                // Flush current file
                file.writer.flush().map_err(PersistenceError::Io)?;

                // Close current file
                *current_file = None;

                // Clean up old files
                self.cleanup_old_files(self.config.max_files).await?;
            }
        }

        Ok(())
    }

    async fn check_flush(&self) -> Result<()> {
        let should_flush = {
            let last_flush = self.last_flush.lock().await;
            last_flush.elapsed() >= self.config.flush_interval
        };

        if should_flush {
            self.flush().await?;
        }

        Ok(())
    }

    fn find_highest_sequence(wal_dir: &Path) -> Result<u64> {
        let mut highest = 0u64;

        if !wal_dir.exists() {
            return Ok(highest);
        }

        let entries = std::fs::read_dir(wal_dir).map_err(PersistenceError::Io)?;

        for entry in entries {
            let entry = entry.map_err(PersistenceError::Io)?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("wal") {
                let (_, last_sequence) = Self::read_sequence_range(&path)?;
                highest = highest.max(last_sequence);
            }
        }

        Ok(highest)
    }

    fn read_sequence_range(path: &Path) -> Result<(u64, u64)> {
        let content = std::fs::read_to_string(path).map_err(PersistenceError::Io)?;

        let mut first_sequence = None;
        let mut last_sequence = 0u64;

        for line in content.lines() {
            if let Ok(entry) = serde_json::from_str::<WalEntry>(line) {
                if first_sequence.is_none() {
                    first_sequence = Some(entry.sequence);
                }
                last_sequence = entry.sequence;
            }
        }

        Ok((first_sequence.unwrap_or(0), last_sequence))
    }
}
