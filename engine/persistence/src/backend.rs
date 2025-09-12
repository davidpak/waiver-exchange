//! Persistence backend trait and implementations

use crate::config::PersistenceConfig;
use crate::error::Result;
use crate::snapshot::{Snapshot, SnapshotManager, SystemState};
use crate::wal::{Wal, WalEntry, WalOperation};
use std::path::PathBuf;
use uuid::Uuid;

/// Abstract trait for persistence backends
#[async_trait::async_trait]
pub trait PersistenceBackend: Send + Sync {
    /// Initialize the persistence backend
    async fn initialize(&mut self) -> Result<()>;

    /// Shutdown the persistence backend
    async fn shutdown(&mut self) -> Result<()>;

    /// Write a WAL entry
    async fn write_wal_entry(&self, operation: WalOperation) -> Result<u64>;

    /// Create a snapshot of the current system state
    async fn create_snapshot(&self, state: SystemState, tick: u64) -> Result<Uuid>;

    /// Load the most recent snapshot
    async fn load_latest_snapshot(&self) -> Result<Option<Snapshot>>;

    /// Load a specific snapshot by ID
    async fn load_snapshot_by_id(&self, snapshot_id: Uuid) -> Result<Option<Snapshot>>;

    /// Get the configuration
    fn config(&self) -> &PersistenceConfig;

    /// Get the data directory
    fn data_dir(&self) -> &PathBuf;
}

/// Local file-based persistence backend
pub struct LocalPersistence {
    config: PersistenceConfig,
    wal: Wal,
    snapshot_manager: SnapshotManager,
    initialized: bool,
}

impl LocalPersistence {
    /// Create a new local persistence backend
    pub fn new(config: PersistenceConfig) -> Result<Self> {
        // Validate configuration
        config.validate().map_err(crate::error::PersistenceError::config)?;

        // Create WAL
        let wal = Wal::new(config.wal.clone(), config.wal_dir())?;

        // Create snapshot manager
        let snapshot_manager =
            SnapshotManager::new(config.snapshot.clone(), config.snapshots_dir())?;

        Ok(Self { config, wal, snapshot_manager, initialized: false })
    }

    /// Create a new local persistence backend with default config
    pub fn with_default_config(data_dir: impl Into<PathBuf>) -> Result<Self> {
        let config = PersistenceConfig::new(data_dir);
        Self::new(config)
    }
}

#[async_trait::async_trait]
impl PersistenceBackend for LocalPersistence {
    async fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        // Ensure data directory exists
        std::fs::create_dir_all(&self.config.data_dir)
            .map_err(crate::error::PersistenceError::Io)?;

        // Ensure subdirectories exist
        std::fs::create_dir_all(self.config.wal_dir())
            .map_err(crate::error::PersistenceError::Io)?;

        std::fs::create_dir_all(self.config.snapshots_dir())
            .map_err(crate::error::PersistenceError::Io)?;

        self.initialized = true;

        tracing::info!("Local persistence backend initialized at: {:?}", self.config.data_dir);

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }

        // Flush WAL
        self.wal.flush().await?;

        // Create final snapshot if configured
        if self.config.snapshot.snapshot_on_shutdown {
            // TODO: Get current system state and create snapshot
            tracing::info!("Creating shutdown snapshot...");
        }

        self.initialized = false;

        tracing::info!("Local persistence backend shutdown complete");

        Ok(())
    }

    async fn write_wal_entry(&self, operation: WalOperation) -> Result<u64> {
        if !self.initialized {
            return Err(crate::error::PersistenceError::invalid_operation(
                "Persistence backend not initialized",
            ));
        }

        self.wal.write_entry(operation).await
    }

    async fn create_snapshot(&self, state: SystemState, tick: u64) -> Result<Uuid> {
        if !self.initialized {
            return Err(crate::error::PersistenceError::invalid_operation(
                "Persistence backend not initialized",
            ));
        }

        self.snapshot_manager.create_snapshot(state, tick).await
    }

    async fn load_latest_snapshot(&self) -> Result<Option<Snapshot>> {
        if !self.initialized {
            return Err(crate::error::PersistenceError::invalid_operation(
                "Persistence backend not initialized",
            ));
        }

        self.snapshot_manager.load_latest_snapshot().await
    }

    async fn load_snapshot_by_id(&self, snapshot_id: Uuid) -> Result<Option<Snapshot>> {
        if !self.initialized {
            return Err(crate::error::PersistenceError::invalid_operation(
                "Persistence backend not initialized",
            ));
        }

        self.snapshot_manager.load_snapshot_by_id(snapshot_id).await
    }

    fn config(&self) -> &PersistenceConfig {
        &self.config
    }

    fn data_dir(&self) -> &PathBuf {
        &self.config.data_dir
    }
}

/// In-memory persistence backend (for testing)
pub struct InMemoryPersistence {
    config: PersistenceConfig,
    wal_entries: std::sync::Arc<tokio::sync::Mutex<Vec<WalEntry>>>,
    snapshots: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<Uuid, Snapshot>>>,
    initialized: bool,
}

impl InMemoryPersistence {
    /// Create a new in-memory persistence backend
    pub fn new(config: PersistenceConfig) -> Self {
        Self {
            config,
            wal_entries: std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new())),
            snapshots: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            initialized: false,
        }
    }

    /// Create a new in-memory persistence backend with default config
    pub fn with_default_config() -> Self {
        let config = PersistenceConfig::default();
        Self::new(config)
    }
}

#[async_trait::async_trait]
impl PersistenceBackend for InMemoryPersistence {
    async fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        self.initialized = true;

        tracing::info!("In-memory persistence backend initialized");

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }

        self.initialized = false;

        tracing::info!("In-memory persistence backend shutdown complete");

        Ok(())
    }

    async fn write_wal_entry(&self, operation: WalOperation) -> Result<u64> {
        if !self.initialized {
            return Err(crate::error::PersistenceError::invalid_operation(
                "Persistence backend not initialized",
            ));
        }

        let mut entries = self.wal_entries.lock().await;
        let sequence = entries.len() as u64 + 1;

        let entry = WalEntry {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            operation,
            sequence,
            checksum: 0, // TODO: Calculate checksum
        };

        entries.push(entry);

        Ok(sequence)
    }

    async fn create_snapshot(&self, state: SystemState, tick: u64) -> Result<Uuid> {
        if !self.initialized {
            return Err(crate::error::PersistenceError::invalid_operation(
                "Persistence backend not initialized",
            ));
        }

        let snapshot = Snapshot {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            tick,
            state,
            metadata: crate::snapshot::SnapshotMetadata {
                version: "1.0".to_string(),
                compression: None,
                file_size: 0,
                creation_duration_ms: 0,
            },
        };

        let snapshot_id = snapshot.id;
        let mut snapshots = self.snapshots.lock().await;
        snapshots.insert(snapshot_id, snapshot);

        Ok(snapshot_id)
    }

    async fn load_latest_snapshot(&self) -> Result<Option<Snapshot>> {
        if !self.initialized {
            return Err(crate::error::PersistenceError::invalid_operation(
                "Persistence backend not initialized",
            ));
        }

        let snapshots = self.snapshots.lock().await;

        // Find the snapshot with the highest tick
        let latest = snapshots.values().max_by_key(|s| s.tick).cloned();

        Ok(latest)
    }

    async fn load_snapshot_by_id(&self, snapshot_id: Uuid) -> Result<Option<Snapshot>> {
        if !self.initialized {
            return Err(crate::error::PersistenceError::invalid_operation(
                "Persistence backend not initialized",
            ));
        }

        let snapshots = self.snapshots.lock().await;
        Ok(snapshots.get(&snapshot_id).cloned())
    }

    fn config(&self) -> &PersistenceConfig {
        &self.config
    }

    fn data_dir(&self) -> &PathBuf {
        &self.config.data_dir
    }
}
