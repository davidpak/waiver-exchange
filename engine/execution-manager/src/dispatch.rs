// Event dispatch for ExecutionManager

use crate::analytics_converter::AnalyticsConverter;
use crate::config::FanoutConfig;
use crate::event::DispatchEvent;
use crate::metrics::MetricsCollector;
// Removed unused import
use std::sync::Arc;
use tokio::sync::mpsc;

/// Event dispatcher for fanning out events to downstream systems
pub struct EventDispatcher {
    #[allow(dead_code)]
    config: FanoutConfig,
    metrics: Arc<MetricsCollector>,

    // Analytics integration
    analytics_converter: AnalyticsConverter,
    analytics_sender: Option<mpsc::UnboundedSender<analytics_engine::analytics::AnalyticsEvent>>,
}

impl EventDispatcher {
    pub fn new(config: FanoutConfig, metrics: Arc<MetricsCollector>) -> Self {
        Self {
            config,
            metrics,
            analytics_converter: AnalyticsConverter::new(100), // Sample every 100 ticks
            analytics_sender: None,
        }
    }

    /// Set up analytics integration
    pub fn set_analytics_sender(
        &mut self,
        sender: mpsc::UnboundedSender<analytics_engine::analytics::AnalyticsEvent>,
    ) {
        self.analytics_sender = Some(sender);
    }

    pub fn dispatch(&self, event: DispatchEvent) -> Result<(), String> {
        // Update metrics
        self.metrics.events_processed_total.inc();

        // Convert to analytics event and send if analytics is enabled
        if let Some(sender) = &self.analytics_sender {
            if let Some(analytics_event) = self.analytics_converter.convert_event(&event) {
                if let Err(e) = sender.send(analytics_event) {
                    // Log error but don't fail the dispatch
                    eprintln!("Failed to send analytics event: {e}");
                }
            }
        }

        // TODO: Dispatch to other systems (ReplayEngine, WebUI, etc.)

        Ok(())
    }

    pub fn dispatch_tick_boundary(&self, event: DispatchEvent) -> Result<(), String> {
        // Update metrics
        self.metrics.ticks_flushed_total.inc();

        // Convert tick boundary to analytics event
        if let Some(sender) = &self.analytics_sender {
            if let Some(analytics_event) = self.analytics_converter.convert_event(&event) {
                if let Err(e) = sender.send(analytics_event) {
                    eprintln!("Failed to send tick boundary analytics event: {e}");
                }
            }
        }

        Ok(())
    }

    pub fn get_queue_stats(&self) -> DispatchStats {
        DispatchStats {
            queue_depth: 0,
            queue_capacity: 0,
            events_dispatched: self.metrics.events_processed_total.get(),
        }
    }

    pub fn shutdown(&self) -> Result<(), String> {
        // TODO: Implement graceful shutdown
        Ok(())
    }
}

/// Dispatch statistics
#[derive(Debug, Clone)]
pub struct DispatchStats {
    pub queue_depth: u64,
    pub queue_capacity: u64,
    pub events_dispatched: u64,
}

/// Fanout destination types
#[derive(Debug, Clone)]
pub enum FanoutDestination {
    ReplayEngine,
    AnalyticsEngine,
    WebUI,
    Custom(String),
}
