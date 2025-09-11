//! # Metrics Aggregation
//! 
//! Real-time aggregation of analytics metrics for efficient storage and querying.

use crate::analytics::AnalyticsEvent;
use crate::config::AggregationConfig;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

/// Metrics aggregator for real-time processing
#[derive(Debug)]
pub struct MetricsAggregator {
    config: AggregationConfig,
    buffers: Arc<Mutex<HashMap<String, AggregationBuffer>>>,
}

/// Aggregation buffer for a specific metric type
#[derive(Debug)]
struct AggregationBuffer {
    events: Vec<AnalyticsEvent>,
    last_flush: std::time::Instant,
    window_size: Duration,
}

impl MetricsAggregator {
    /// Create new metrics aggregator
    pub fn new(config: AggregationConfig) -> Self {
        Self {
            config,
            buffers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Start the aggregation service
    pub async fn start(&self) -> Result<()> {
        tracing::info!("Starting metrics aggregation service");
        
        // Start aggregation loop
        let mut interval = interval(Duration::from_secs(1));
        let buffers = self.buffers.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            loop {
                interval.tick().await;
                
                // Process all buffers
                let mut buffers_guard = buffers.lock().await;
                let mut to_remove = Vec::new();
                
                for (key, buffer) in buffers_guard.iter_mut() {
                    if buffer.should_flush() {
                        // Flush buffer
                        if let Err(e) = Self::flush_buffer(buffer, &config).await {
                            tracing::error!("Failed to flush buffer {}: {}", key, e);
                        }
                        to_remove.push(key.clone());
                    }
                }
                
                // Remove flushed buffers
                for key in to_remove {
                    buffers_guard.remove(&key);
                }
            }
        });
        
        Ok(())
    }
    
    /// Add event to aggregation buffer
    pub async fn add_event(&self, event: AnalyticsEvent) -> Result<()> {
        let buffer_key = self.get_buffer_key(&event);
        let mut buffers = self.buffers.lock().await;
        
        let buffer = buffers.entry(buffer_key.clone()).or_insert_with(|| {
            AggregationBuffer {
                events: Vec::new(),
                last_flush: std::time::Instant::now(),
                window_size: Duration::from_secs(1), // Default 1 second window
            }
        });
        
        buffer.events.push(event);
        
        // Check if buffer should be flushed
        if buffer.should_flush() {
            Self::flush_buffer(buffer, &self.config).await?;
            buffers.remove(&buffer_key);
        }
        
        Ok(())
    }
    
    /// Get buffer key for event
    fn get_buffer_key(&self, event: &AnalyticsEvent) -> String {
        format!("{}_{}", event.symbol, event.event_type() as i32)
    }
    
    /// Flush aggregation buffer
    async fn flush_buffer(
        buffer: &mut AggregationBuffer,
        _config: &AggregationConfig,
    ) -> Result<()> {
        if buffer.events.is_empty() {
            return Ok(());
        }
        
        // Perform aggregation
        let aggregated_events = Self::aggregate_events(&buffer.events)?;
        
        // Store aggregated events (this would integrate with storage)
        tracing::debug!("Flushing {} events, aggregated to {} events", 
                       buffer.events.len(), aggregated_events.len());
        
        // Clear buffer
        buffer.events.clear();
        buffer.last_flush = std::time::Instant::now();
        
        Ok(())
    }
    
    /// Aggregate events within a time window
    fn aggregate_events(events: &[AnalyticsEvent]) -> Result<Vec<AnalyticsEvent>> {
        if events.is_empty() {
            return Ok(Vec::new());
        }
        
        // Group events by type
        let mut performance_events = Vec::new();
        let mut business_events = Vec::new();
        let mut health_events = Vec::new();
        let mut operational_events = Vec::new();
        
        for event in events {
            match event.event_type() {
                crate::analytics::EventType::Performance => {
                    performance_events.push(event);
                }
                crate::analytics::EventType::Business => {
                    business_events.push(event);
                }
                crate::analytics::EventType::SystemHealth => {
                    health_events.push(event);
                }
                crate::analytics::EventType::Operational => {
                    operational_events.push(event);
                }
            }
        }
        
        let mut aggregated = Vec::new();
        
        // Aggregate each type
        if !performance_events.is_empty() {
            aggregated.push(Self::aggregate_performance_events(&performance_events)?);
        }
        if !business_events.is_empty() {
            aggregated.push(Self::aggregate_business_events(&business_events)?);
        }
        if !health_events.is_empty() {
            aggregated.push(Self::aggregate_health_events(&health_events)?);
        }
        if !operational_events.is_empty() {
            aggregated.push(Self::aggregate_operational_events(&operational_events)?);
        }
        
        Ok(aggregated)
    }
    
    /// Aggregate performance events
    fn aggregate_performance_events(events: &[&AnalyticsEvent]) -> Result<AnalyticsEvent> {
        if events.is_empty() {
            return Err(anyhow::anyhow!("No events to aggregate"));
        }
        
        let first_event = events[0];
        let mut total_tick_duration = 0u64;
        let mut total_latency = 0u64;
        let mut total_queue_depth = 0u32;
        let mut total_memory = 0u64;
        let mut total_cpu = 0.0;
        let mut total_threads = 0u32;
        let mut count = 0;
        
        for event in events {
            if let Some(data) = &event.data {
                if let crate::analytics::analytics_event::Data::Performance(perf) = data {
                    total_tick_duration += perf.tick_duration_ns;
                    total_latency += perf.event_processing_latency_ns;
                    total_queue_depth += perf.queue_depth;
                    total_memory += perf.memory_usage_bytes;
                    total_cpu += perf.cpu_utilization_percent;
                    total_threads += perf.thread_count;
                    count += 1;
                }
            }
        }
        
        let aggregated_perf = crate::analytics::PerformanceMetrics {
            tick_duration_ns: total_tick_duration / count as u64,
            event_processing_latency_ns: total_latency / count as u64,
            queue_depth: total_queue_depth / count,
            memory_usage_bytes: total_memory / count as u64,
            cpu_utilization_percent: total_cpu / count as f64,
            thread_count: total_threads / count,
        };
        
        Ok(AnalyticsEvent {
            timestamp_ns: first_event.timestamp_ns,
            tick_id: first_event.tick_id,
            symbol: first_event.symbol.clone(),
            event_type: first_event.event_type,
            data: Some(crate::analytics::analytics_event::Data::Performance(aggregated_perf)),
        })
    }
    
    /// Aggregate business events
    fn aggregate_business_events(events: &[&AnalyticsEvent]) -> Result<AnalyticsEvent> {
        if events.is_empty() {
            return Err(anyhow::anyhow!("No events to aggregate"));
        }
        
        let first_event = events[0];
        let mut total_orders = 0u32;
        let mut total_trades = 0u32;
        let mut total_volume = 0u64;
        let mut total_accounts = 0u32;
        let mut total_book_depth = 0u32;
        let mut total_trade_size = 0.0;
        let mut count = 0;
        
        for event in events {
            if let Some(data) = &event.data {
                if let crate::analytics::analytics_event::Data::Business(biz) = data {
                    total_orders += biz.orders_processed;
                    total_trades += biz.trades_executed;
                    total_volume += biz.volume_traded;
                    total_accounts += biz.active_accounts;
                    total_book_depth += biz.order_book_depth;
                    total_trade_size += biz.average_trade_size;
                    count += 1;
                }
            }
        }
        
        let aggregated_biz = crate::analytics::BusinessMetrics {
            orders_processed: total_orders,
            trades_executed: total_trades,
            volume_traded: total_volume,
            active_accounts: total_accounts,
            order_book_depth: total_book_depth,
            average_trade_size: total_trade_size / count as f64,
        };
        
        Ok(AnalyticsEvent {
            timestamp_ns: first_event.timestamp_ns,
            tick_id: first_event.tick_id,
            symbol: first_event.symbol.clone(),
            event_type: first_event.event_type,
            data: Some(crate::analytics::analytics_event::Data::Business(aggregated_biz)),
        })
    }
    
    /// Aggregate health events
    fn aggregate_health_events(events: &[&AnalyticsEvent]) -> Result<AnalyticsEvent> {
        if events.is_empty() {
            return Err(anyhow::anyhow!("No events to aggregate"));
        }
        
        let first_event = events[0];
        let mut total_crashes = 0u32;
        let mut total_overflows = 0u32;
        let mut total_memory_failures = 0u32;
        let mut total_error_rate = 0.0;
        let mut total_uptime = 0u64;
        let mut error_messages = Vec::new();
        let mut count = 0;
        
        for event in events {
            if let Some(data) = &event.data {
                if let crate::analytics::analytics_event::Data::Health(health) = data {
                    if health.engine_crashed {
                        total_crashes += 1;
                    }
                    total_overflows += health.queue_overflows;
                    total_memory_failures += health.memory_allocation_failures;
                    total_error_rate += health.error_rate_percent;
                    total_uptime += health.uptime_seconds;
                    if !health.error_message.is_empty() {
                        error_messages.push(health.error_message.clone());
                    }
                    count += 1;
                }
            }
        }
        
        let aggregated_health = crate::analytics::SystemHealthMetrics {
            engine_crashed: total_crashes > 0,
            queue_overflows: total_overflows,
            memory_allocation_failures: total_memory_failures,
            error_rate_percent: total_error_rate / count as f64,
            uptime_seconds: total_uptime / count as u64,
            error_message: error_messages.join("; "),
        };
        
        Ok(AnalyticsEvent {
            timestamp_ns: first_event.timestamp_ns,
            tick_id: first_event.tick_id,
            symbol: first_event.symbol.clone(),
            event_type: first_event.event_type,
            data: Some(crate::analytics::analytics_event::Data::Health(aggregated_health)),
        })
    }
    
    /// Aggregate operational events
    fn aggregate_operational_events(events: &[&AnalyticsEvent]) -> Result<AnalyticsEvent> {
        if events.is_empty() {
            return Err(anyhow::anyhow!("No events to aggregate"));
        }
        
        let first_event = events[0];
        let mut total_activations = 0u32;
        let mut total_evictions = 0u32;
        let mut total_thread_util = 0.0;
        let mut total_network_bytes = 0u64;
        let mut total_disk_bytes = 0u64;
        let mut total_active_symbols = 0u32;
        let mut count = 0;
        
        for event in events {
            if let Some(data) = &event.data {
                if let crate::analytics::analytics_event::Data::Operational(op) = data {
                    if op.symbol_activated {
                        total_activations += 1;
                    }
                    if op.symbol_evicted {
                        total_evictions += 1;
                    }
                    total_thread_util += op.thread_utilization_percent;
                    total_network_bytes += op.network_bytes_sent;
                    total_disk_bytes += op.disk_bytes_written;
                    total_active_symbols += op.active_symbols;
                    count += 1;
                }
            }
        }
        
        let aggregated_op = crate::analytics::OperationalMetrics {
            symbol_activated: total_activations > 0,
            symbol_evicted: total_evictions > 0,
            thread_utilization_percent: total_thread_util / count as f64,
            network_bytes_sent: total_network_bytes,
            disk_bytes_written: total_disk_bytes,
            active_symbols: total_active_symbols / count,
        };
        
        Ok(AnalyticsEvent {
            timestamp_ns: first_event.timestamp_ns,
            tick_id: first_event.tick_id,
            symbol: first_event.symbol.clone(),
            event_type: first_event.event_type,
            data: Some(crate::analytics::analytics_event::Data::Operational(aggregated_op)),
        })
    }
}

impl AggregationBuffer {
    /// Check if buffer should be flushed
    fn should_flush(&self) -> bool {
        self.events.len() >= 100 || // Max 100 events per buffer
        self.last_flush.elapsed() >= self.window_size
    }
}
