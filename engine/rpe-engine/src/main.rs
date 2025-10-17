use rpe_engine::{RpeConfig, RpeEngine};
use tracing::{info, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("🚀 RPE Engine starting with proven test script logic...");
    info!("Starting RPE Engine");
    
    // Load configuration
    println!("📋 Loading configuration...");
    let config = RpeConfig::from_env()?;
    println!("✅ Configuration loaded successfully");
    info!("Loaded configuration: {:?}", config);
    info!("Database URL: {}", config.database.url);
    info!("Season: {}", config.rpe.season);
    
    // Create RPE engine
    println!("🔧 Creating RPE engine...");
    let mut engine = RpeEngine::new(config).await?;
    println!("✅ RPE engine created successfully");
    info!("Created RPE engine instance");
    
    // Process Fair Price 2.0 using the proven test script logic
    println!("🔄 Processing Fair Price 2.0 for ALL players...");
    info!("Processing Fair Price 2.0 using proven test script logic...");
    
    match engine.process_fair_price_2_0().await {
        Ok(events) => {
            println!("✅ Successfully processed Fair Price 2.0 for all players!");
            info!("✅ Successfully processed Fair Price 2.0");
            
            let mut fair_price_events = 0;
            let mut batch_events = 0;
            
            for event in events {
                match event {
                    rpe_engine::RpeEvent::FairPriceUpdated { player_id, fair_cents, delta_cents, .. } => {
                        fair_price_events += 1;
                        if fair_price_events <= 10 { // Show first 10 updates
                            let price_dollars = fair_cents as f64 / 100.0;
                            println!("  Player {}: ${:.2} (Δ: {} cents)", player_id, price_dollars, delta_cents);
                        }
                    }
                    rpe_engine::RpeEvent::BatchCompleted { processed_count, updated_count, .. } => {
                        batch_events += 1;
                        println!("  Batch {}: processed {}, updated {}", batch_events, processed_count, updated_count);
                    }
                    _ => {}
                }
            }
            
            println!("Total fair price events: {}", fair_price_events);
            info!("Total fair price events: {}", fair_price_events);
        }
        Err(e) => {
            println!("❌ Failed to process Fair Price 2.0: {}", e);
            error!("❌ Failed to process Fair Price 2.0: {}", e);
            return Err(e);
        }
    }
    
    println!("🎉 RPE Engine processing completed successfully!");
    info!("RPE Engine processing completed!");
    Ok(())
}