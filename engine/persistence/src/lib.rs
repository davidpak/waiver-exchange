//! # Persistence Layer
//!
//! This crate provides the persistence infrastructure for the Waiver Exchange trading system.
//! It implements a Write-Ahead Log (WAL) + Snapshots strategy for durable order book storage.
//!
//! ## Architecture
//!
//! - **PersistenceBackend**: Abstract trait for different storage backends
//! - **LocalPersistence**: Local file-based implementation
//! - **WAL**: Write-ahead log for all order book changes
//! - **Snapshots**: Periodic state captures for fast recovery
//!
//! ## Usage
//!
//! ```rust
//! use persistence::{PersistenceBackend, create_local_persistence};
//! use tempfile::TempDir;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let temp_dir = TempDir::new()?;
//!     let mut persistence = create_local_persistence(temp_dir.path())?;
//!     persistence.initialize().await?;
//!     
//!     // Write operations to WAL
//!     use persistence::wal::WalOperation;
//!     let operation = WalOperation::SubmitOrder {
//!         symbol_id: 1,
//!         account_id: 100,
//!         side: "buy".to_string(),
//!         order_type: "limit".to_string(),
//!         price: Some(15000),
//!         quantity: 100,
//!         order_id: 1,
//!     };
//!     persistence.write_wal_entry(operation).await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod backend;
pub mod config;
pub mod error;
pub mod local;
pub mod snapshot;
pub mod wal;

pub use backend::{InMemoryPersistence, LocalPersistence, PersistenceBackend};
pub use config::PersistenceConfig;
pub use error::{PersistenceError, Result};
pub use local::{create_local_persistence, create_local_persistence_with_config};
pub use wal::WalOperation;

pub use chrono::{DateTime, Utc};
/// Re-export common types for convenience
pub use serde::{Deserialize, Serialize};
pub use uuid::Uuid;
