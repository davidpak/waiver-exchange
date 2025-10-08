use rpe_engine::{RpeConfig, RpeEngine};
use tracing::{info, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Starting RPE Engine");
    
    // Load configuration
    let config = RpeConfig::from_env()?;
    info!("Loaded configuration: {:?}", config);
    
    // Create RPE engine
    let mut engine = RpeEngine::new(config).await?;
    info!("Created RPE engine instance");
    
    // Process season projections for initial F₀ calculations
    info!("Processing season projections...");
    match engine.process_season_projections().await {
        Ok(events) => {
            info!("✅ Successfully processed season projections");
            for event in events {
                info!("Event: {:?}", event);
            }
        }
        Err(e) => {
            error!("❌ Failed to process season projections: {}", e);
            return Err(e);
        }
    }
    
    // Process player week points for current week
    let current_week = 4; // TODO: Get from config
    info!("Processing player week points for week {}...", current_week);
    match engine.process_player_week_points(current_week).await {
        Ok(events) => {
            info!("✅ Successfully processed player week points");
            for event in events {
                info!("Event: {:?}", event);
            }
        }
        Err(e) => {
            error!("❌ Failed to process player week points: {}", e);
            return Err(e);
        }
    }
    
    info!("RPE Engine processing completed!");
    Ok(())
}
