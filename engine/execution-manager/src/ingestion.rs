// Event ingestion for ExecutionManager

use crate::event::DispatchEvent;
use crate::metrics::MetricsCollector;
use std::sync::Arc;

/// Event ingestion component
pub struct EventIngestion {
    metrics: Arc<MetricsCollector>,
}

impl EventIngestion {
    pub fn new(metrics: Arc<MetricsCollector>) -> Self {
        Self { metrics }
    }

    pub fn ingest(&self, _event: DispatchEvent) -> Result<(), String> {
        // TODO: Implement event ingestion logic
        self.metrics.events_processed_total.inc();
        Ok(())
    }
}

/// Ingestion statistics
#[derive(Debug, Clone)]
pub struct IngestionStats {
    pub events_processed: u64,
    pub events_dropped: u64,
    pub processing_latency_ns: u64,
}

impl EventIngestion {
    pub fn get_stats(&self) -> IngestionStats {
        IngestionStats {
            events_processed: self.metrics.events_processed_total.get(),
            events_dropped: self.metrics.events_dropped_total.get(),
            processing_latency_ns: self.metrics.processing_latency.percentile(50.0),
        }
    }
}
