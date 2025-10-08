use rpe_engine::{RpeConfig, RpeEngine};
use tracing::{info, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Testing RPE Engine");
    
    // Load configuration
    let config = RpeConfig::from_env()?;
    info!("Loaded configuration");
    
    // Create RPE engine
    let mut engine = RpeEngine::new(config).await?;
    info!("Created RPE engine instance");
    
    // Test processing season projections (pre-game F₀ prices)
    info!("Testing season projections processing (pre-game F₀ prices)...");
    match engine.process_season_projections().await {
        Ok(events) => {
            info!("✅ Successfully processed season projections");
            let mut fair_price_events = 0;
            let mut batch_events = 0;
            
            for event in events {
                match event {
                    rpe_engine::RpeEvent::FairPriceUpdated { player_id, fair_cents, .. } => {
                        fair_price_events += 1;
                        if fair_price_events <= 3 {
                            info!("  Player {}: {} cents", player_id, fair_cents);
                        }
                    }
                    rpe_engine::RpeEvent::BatchCompleted { processed_count, updated_count, .. } => {
                        batch_events += 1;
                        info!("  Batch {}: processed {}, updated {}", batch_events, processed_count, updated_count);
                    }
                    _ => {}
                }
            }
            
            info!("Total fair price events: {}", fair_price_events);
        }
        Err(e) => {
            error!("❌ Failed to process season projections: {}", e);
        }
    }
    
    // Test processing player week points (in-game Fₜ updates)
    let current_week = 4;
    info!("Testing player week points processing for week {} (in-game Fₜ updates)...", current_week);
    match engine.process_player_week_points(current_week).await {
        Ok(events) => {
            info!("✅ Successfully processed player week points");
            let mut fair_price_events = 0;
            let mut batch_events = 0;
            
            for event in events {
                match event {
                    rpe_engine::RpeEvent::FairPriceUpdated { player_id, fair_cents, .. } => {
                        fair_price_events += 1;
                        if fair_price_events <= 3 {
                            info!("  Player {}: {} cents", player_id, fair_cents);
                        }
                    }
                    rpe_engine::RpeEvent::BatchCompleted { processed_count, updated_count, .. } => {
                        batch_events += 1;
                        info!("  Batch {}: processed {}, updated {}", batch_events, processed_count, updated_count);
                    }
                    _ => {}
                }
            }
            
            info!("Total fair price events: {}", fair_price_events);
        }
        Err(e) => {
            error!("❌ Failed to process player week points: {}", e);
        }
    }
    
    info!("RPE Engine test completed!");
    Ok(())
}
