//! # Storage Implementation
//!
//! Storage for analytics data (simplified version without Parquet for now).

use crate::analytics::AnalyticsEvent;
use crate::config::StorageConfig;
use anyhow::Result;
use serde_json;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;

/// Storage implementation (simplified)
#[derive(Debug, Clone)]
pub struct ParquetStorage {
    #[allow(dead_code)]
    config: StorageConfig,
    data: Arc<Mutex<HashMap<String, Vec<AnalyticsEvent>>>>,
}

impl ParquetStorage {
    /// Create new storage
    pub async fn new(base_path: &Path) -> Result<Self> {
        // Ensure base directory exists
        fs::create_dir_all(base_path).await?;

        Ok(Self { config: StorageConfig::default(), data: Arc::new(Mutex::new(HashMap::new())) })
    }

    /// Store analytics events
    pub async fn store_events(&self, events: Vec<AnalyticsEvent>) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let mut data = self.data.lock().await;

        // Group events by type and symbol
        for event in events {
            let key = format!("{}_{}", event.symbol, event.event_type);
            data.entry(key).or_insert_with(Vec::new).push(event);
        }

        // In a real implementation, we would write to Parquet files here
        tracing::debug!(
            "Stored {} events in memory",
            data.values().map(|v| v.len()).sum::<usize>()
        );

        Ok(())
    }

    /// Get stored events (for testing/querying)
    pub async fn get_events(&self, symbol: &str, event_type: i32) -> Result<Vec<AnalyticsEvent>> {
        let data = self.data.lock().await;
        let key = format!("{symbol}_{event_type}");
        Ok(data.get(&key).cloned().unwrap_or_default())
    }

    /// Get all stored events
    pub async fn get_all_events(&self) -> Result<Vec<AnalyticsEvent>> {
        let data = self.data.lock().await;
        Ok(data.values().flatten().cloned().collect())
    }

    /// Export data to JSON (for debugging)
    pub async fn export_to_json(&self, path: &Path) -> Result<()> {
        let data = self.data.lock().await;
        let json = serde_json::to_string_pretty(&*data)?;
        fs::write(path, json).await?;
        Ok(())
    }
}
