//! # Analytics Engine Demo
//! 
//! Demonstrates the AnalyticsEngine functionality with sample data.

use analytics_engine::{
    AnalyticsConfig, init_analytics,
    analytics::{AnalyticsEvent, EventType, PerformanceMetrics, BusinessMetrics, SystemHealthMetrics, OperationalMetrics}
};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("🚀 Starting AnalyticsEngine Demo");
    
    // Create configuration
    let config = AnalyticsConfig::default();
    
    // Initialize analytics engine
    let analytics = init_analytics(config).await?;
    
    // Start the engine
    let _engine_handle = tokio::spawn(async move {
        analytics.start().await
    });
    
    // Generate sample events
    generate_sample_events().await?;
    
    // Wait a bit for processing
    sleep(Duration::from_secs(2)).await;
    
    println!("✅ Demo completed successfully!");
    
    // Note: In a real implementation, we'd gracefully shutdown
    // For demo purposes, we'll just exit
    Ok(())
}

async fn generate_sample_events() -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 Generating sample analytics events...");
    
    // Generate performance events
    for i in 0..10 {
        let event = AnalyticsEvent {
            timestamp_ns: current_timestamp_ns(),
            tick_id: i,
            symbol: "AAPL".to_string(),
            event_type: EventType::Performance as i32,
            data: Some(analytics_engine::analytics::analytics_event::Data::Performance(
                PerformanceMetrics {
                    tick_duration_ns: 1000 + (i * 100),
                    event_processing_latency_ns: 500 + (i * 50),
                    queue_depth: 10 + (i as u32),
                    memory_usage_bytes: 1024 * 1024 * (i as u64 + 1),
                    cpu_utilization_percent: 50.0 + (i as f64 * 5.0),
                    thread_count: 4 + (i as u32),
                }
            )),
        };
        
        // In a real implementation, we'd send this to the analytics engine
        if let Some(data) = &event.data {
            if let analytics_engine::analytics::analytics_event::Data::Performance(perf) = data {
                println!("  📈 Performance event: tick_duration={}ns, cpu={}%", 
                        perf.tick_duration_ns, perf.cpu_utilization_percent);
            }
        }
        
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    // Generate business events
    for i in 0..5 {
        let event = AnalyticsEvent {
            timestamp_ns: current_timestamp_ns(),
            tick_id: i,
            symbol: "AAPL".to_string(),
            event_type: EventType::Business as i32,
            data: Some(analytics_engine::analytics::analytics_event::Data::Business(
                BusinessMetrics {
                    orders_processed: 100 + (i as u32 * 10),
                    trades_executed: 50 + (i as u32 * 5),
                    volume_traded: 1000 + (i as u64 * 100),
                    active_accounts: 25 + (i as u32),
                    order_book_depth: 20 + (i as u32),
                    average_trade_size: 10.0 + (i as f64),
                }
            )),
        };
        
        if let Some(data) = &event.data {
            if let analytics_engine::analytics::analytics_event::Data::Business(biz) = data {
                println!("  💰 Business event: orders={}, trades={}, volume={}", 
                        biz.orders_processed, biz.trades_executed, biz.volume_traded);
            }
        }
        
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    
    // Generate system health events
    for i in 0..3 {
        let event = AnalyticsEvent {
            timestamp_ns: current_timestamp_ns(),
            tick_id: i,
            symbol: "AAPL".to_string(),
            event_type: EventType::SystemHealth as i32,
            data: Some(analytics_engine::analytics::analytics_event::Data::Health(
                SystemHealthMetrics {
                    engine_crashed: i == 1, // Simulate one crash
                    queue_overflows: if i == 2 { 5 } else { 0 },
                    memory_allocation_failures: 0,
                    error_rate_percent: if i == 1 { 5.0 } else { 0.1 },
                    uptime_seconds: 3600 + (i * 1800),
                    error_message: if i == 1 { "Simulated crash for demo".to_string() } else { String::new() },
                }
            )),
        };
        
        if let Some(data) = &event.data {
            if let analytics_engine::analytics::analytics_event::Data::Health(health) = data {
                println!("  🏥 Health event: crashed={}, error_rate={}%", 
                        health.engine_crashed, health.error_rate_percent);
            }
        }
        
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    
    // Generate operational events
    for i in 0..2 {
        let event = AnalyticsEvent {
            timestamp_ns: current_timestamp_ns(),
            tick_id: i,
            symbol: "AAPL".to_string(),
            event_type: EventType::Operational as i32,
            data: Some(analytics_engine::analytics::analytics_event::Data::Operational(
                OperationalMetrics {
                    symbol_activated: i == 0,
                    symbol_evicted: i == 1,
                    thread_utilization_percent: 75.0 + (i as f64 * 10.0),
                    network_bytes_sent: 1024 * (i as u64 + 1),
                    disk_bytes_written: 2048 * (i as u64 + 1),
                    active_symbols: 10 + (i as u32),
                }
            )),
        };
        
        if let Some(data) = &event.data {
            if let analytics_engine::analytics::analytics_event::Data::Operational(op) = data {
                println!("  🔧 Operational event: activated={}, evicted={}, threads={}%", 
                        op.symbol_activated, op.symbol_evicted, op.thread_utilization_percent);
            }
        }
        
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    
    println!("✅ Generated {} sample events", 10 + 5 + 3 + 2);
    Ok(())
}

fn current_timestamp_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}
