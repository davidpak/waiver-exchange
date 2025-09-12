//! Local file-based persistence implementation

use crate::backend::LocalPersistence;
use crate::config::PersistenceConfig;
use crate::error::Result;

/// Create a new local persistence instance with default configuration
pub fn create_local_persistence(
    data_dir: impl Into<std::path::PathBuf>,
) -> Result<LocalPersistence> {
    LocalPersistence::with_default_config(data_dir)
}

/// Create a new local persistence instance with custom configuration
pub fn create_local_persistence_with_config(config: PersistenceConfig) -> Result<LocalPersistence> {
    LocalPersistence::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::PersistenceBackend;
    use crate::snapshot::{SystemConfig, SystemState, SystemStats};
    use crate::wal::WalOperation;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_local_persistence_creation() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        let persistence = create_local_persistence(data_dir).unwrap();
        assert_eq!(persistence.data_dir(), &temp_dir.path().to_path_buf());
    }

    #[tokio::test]
    async fn test_local_persistence_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        let mut persistence = create_local_persistence(data_dir).unwrap();
        persistence.initialize().await.unwrap();

        // Verify directories were created
        assert!(persistence.data_dir().exists());
        assert!(persistence.data_dir().join("wal").exists());
        assert!(persistence.data_dir().join("snapshots").exists());
    }

    #[tokio::test]
    async fn test_wal_entry_writing() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        let mut persistence = create_local_persistence(data_dir).unwrap();
        persistence.initialize().await.unwrap();

        let operation = WalOperation::SubmitOrder {
            symbol_id: 1,
            account_id: 100,
            side: "buy".to_string(),
            order_type: "limit".to_string(),
            price: Some(15000),
            quantity: 100,
            order_id: 1,
        };

        let sequence = persistence.write_wal_entry(operation).await.unwrap();
        assert_eq!(sequence, 1);
    }

    #[tokio::test]
    async fn test_snapshot_creation() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        let mut persistence = create_local_persistence(data_dir).unwrap();
        persistence.initialize().await.unwrap();

        let state = SystemState {
            order_books: HashMap::new(),
            accounts: HashMap::new(),
            active_symbols: Vec::new(),
            config: SystemConfig {
                max_symbols: 100,
                max_accounts: 1000,
                tick_duration_ns: 1_000_000,
            },
            stats: SystemStats {
                total_orders: 0,
                total_trades: 0,
                total_volume: 0,
                current_tick: 0,
                uptime_seconds: 0,
            },
        };

        let snapshot_id = persistence.create_snapshot(state, 100).await.unwrap();
        assert!(!snapshot_id.is_nil());
    }

    #[tokio::test]
    async fn test_snapshot_loading() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        let mut persistence = create_local_persistence(data_dir).unwrap();
        persistence.initialize().await.unwrap();

        let state = SystemState {
            order_books: HashMap::new(),
            accounts: HashMap::new(),
            active_symbols: Vec::new(),
            config: SystemConfig {
                max_symbols: 100,
                max_accounts: 1000,
                tick_duration_ns: 1_000_000,
            },
            stats: SystemStats {
                total_orders: 0,
                total_trades: 0,
                total_volume: 0,
                current_tick: 0,
                uptime_seconds: 0,
            },
        };

        let snapshot_id = persistence.create_snapshot(state, 100).await.unwrap();

        let loaded_snapshot = persistence.load_snapshot_by_id(snapshot_id).await.unwrap();
        assert!(loaded_snapshot.is_some());

        let snapshot = loaded_snapshot.unwrap();
        assert_eq!(snapshot.id, snapshot_id);
        assert_eq!(snapshot.tick, 100);
    }

    #[tokio::test]
    async fn test_recovery_with_symbols() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        let mut persistence = create_local_persistence(data_dir).unwrap();
        persistence.initialize().await.unwrap();

        // Create a snapshot with active symbols
        let state = SystemState {
            order_books: HashMap::new(),
            accounts: HashMap::new(),
            active_symbols: vec![1, 2, 3], // Three active symbols
            config: SystemConfig {
                max_symbols: 100,
                max_accounts: 1000,
                tick_duration_ns: 1_000_000,
            },
            stats: SystemStats {
                total_orders: 0,
                total_trades: 0,
                total_volume: 0,
                current_tick: 1000,
                uptime_seconds: 0,
            },
        };

        let snapshot_id = persistence.create_snapshot(state, 1000).await.unwrap();
        println!("Created snapshot: {snapshot_id}");

        // Test loading the latest snapshot
        let loaded_snapshot = persistence.load_latest_snapshot().await.unwrap();
        assert!(loaded_snapshot.is_some());

        let snapshot = loaded_snapshot.unwrap();
        assert_eq!(snapshot.tick, 1000);
        assert_eq!(snapshot.state.active_symbols.len(), 3);
        assert_eq!(snapshot.state.active_symbols, vec![1, 2, 3]);

        println!(
            "✅ Recovery test passed! Found {} symbols: {:?}",
            snapshot.state.active_symbols.len(),
            snapshot.state.active_symbols
        );
    }
}
