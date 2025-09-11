//! # Event Ingestion
//! 
//! Handles ingestion of analytics events from ExecutionManager.

use crate::analytics::AnalyticsEvent;
use crate::config::IngestionConfig;
use anyhow::Result;
use tokio::sync::mpsc;

/// Analytics ingestion service
#[derive(Debug)]
pub struct AnalyticsIngestion {
    config: IngestionConfig,
    event_sender: mpsc::UnboundedSender<AnalyticsEvent>,
}

impl AnalyticsIngestion {
    /// Create new analytics ingestion service
    pub async fn new(config: IngestionConfig) -> Result<Self> {
        let (event_sender, _event_receiver) = mpsc::unbounded_channel();
        
        Ok(Self {
            config,
            event_sender,
        })
    }
    
    /// Start the ingestion service
    pub async fn start(&self) -> Result<()> {
        tracing::info!("Starting analytics ingestion service");
        // Service would run here
        Ok(())
    }
    
    /// Get event sender for external use
    pub fn event_sender(&self) -> mpsc::UnboundedSender<AnalyticsEvent> {
        self.event_sender.clone()
    }
    
    /// Ingest a single event
    pub async fn ingest_event(&self, event: AnalyticsEvent) -> Result<bool> {
        match self.event_sender.send(event) {
            Ok(_) => Ok(true),
            Err(e) => {
                tracing::error!("Failed to send event: {}", e);
                Ok(false)
            }
        }
    }
    
    /// Ingest a batch of events
    pub async fn ingest_batch(&self, events: Vec<AnalyticsEvent>) -> Result<usize> {
        let mut processed = 0;
        
        for event in events {
            if self.ingest_event(event).await? {
                processed += 1;
            }
        }
        
        Ok(processed)
    }
}
