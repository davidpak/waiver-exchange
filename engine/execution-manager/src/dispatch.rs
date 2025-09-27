// Event dispatch for ExecutionManager

use crate::analytics_converter::AnalyticsConverter;
use crate::config::FanoutConfig;
use crate::event::DispatchEvent;
use crate::metrics::MetricsCollector;
use account_service::position::TradeSide;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;
use whistle::TickId;

/// Trait for services that need to be notified after trade settlement
#[async_trait]
pub trait PostSettlementCallback: Send + Sync {
    async fn on_trade_settled(
        &self, 
        account_id: i64, 
        symbol_id: u32, 
        side: TradeSide, 
        quantity: i64, 
        price: i64
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    async fn on_price_updated(&self, symbol_id: u32, price: i64) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    async fn on_tick_complete(&self, tick_id: TickId) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Event dispatcher for fanning out events to downstream systems
pub struct EventDispatcher {
    #[allow(dead_code)]
    config: FanoutConfig,
    metrics: Arc<MetricsCollector>,

    // Analytics integration
    analytics_converter: AnalyticsConverter,
    analytics_sender: Option<mpsc::UnboundedSender<analytics_engine::analytics::AnalyticsEvent>>,
    
    // Post-settlement callbacks
    post_settlement_callbacks: Vec<Arc<dyn PostSettlementCallback>>,
}

impl EventDispatcher {
    pub fn new(config: FanoutConfig, metrics: Arc<MetricsCollector>) -> Self {
        Self {
            config,
            metrics,
            analytics_converter: AnalyticsConverter::new(100), // Sample every 100 ticks
            analytics_sender: None,
            post_settlement_callbacks: Vec::new(),
        }
    }

    /// Set up analytics integration
    pub fn set_analytics_sender(
        &mut self,
        sender: mpsc::UnboundedSender<analytics_engine::analytics::AnalyticsEvent>,
    ) {
        self.analytics_sender = Some(sender);
    }

    /// Add a post-settlement callback
    pub fn add_post_settlement_callback(&mut self, callback: Arc<dyn PostSettlementCallback>) {
        self.post_settlement_callbacks.push(callback);
    }

    /// Notify all callbacks of a trade settlement
    pub async fn notify_trade_settled(
        &self, 
        account_id: i64, 
        symbol_id: u32, 
        side: TradeSide, 
        quantity: i64, 
        price: i64
    ) {
        tracing::info!("📢 Dispatcher: Notifying {} callbacks for trade settlement", self.post_settlement_callbacks.len());
        for (i, callback) in self.post_settlement_callbacks.iter().enumerate() {
            tracing::info!("📢 Dispatcher: Calling callback {} for account {} trade", i, account_id);
            if let Err(e) = callback.on_trade_settled(account_id, symbol_id, side, quantity, price).await {
                tracing::warn!("Post-settlement callback {} failed: {}", i, e);
            } else {
                tracing::info!("📢 Dispatcher: Callback {} completed successfully", i);
            }
        }
    }

    /// Notify all callbacks of a price update
    pub async fn notify_price_updated(&self, symbol_id: u32, price: i64) {
        for callback in &self.post_settlement_callbacks {
            if let Err(e) = callback.on_price_updated(symbol_id, price).await {
                tracing::warn!("Price update callback failed: {}", e);
            }
        }
    }

    /// Notify all callbacks of tick completion
    pub async fn notify_tick_complete(&self, tick_id: TickId) {
        for callback in &self.post_settlement_callbacks {
            if let Err(e) = callback.on_tick_complete(tick_id).await {
                tracing::warn!("Tick complete callback failed: {}", e);
            }
        }
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
