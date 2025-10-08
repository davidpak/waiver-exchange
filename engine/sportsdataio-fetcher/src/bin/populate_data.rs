use sportsdataio_fetcher::{FetcherConfig, SportsDataIOFetcher};
use tracing::{info, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Populating database with initial SportsDataIO data");
    
    // Load configuration
    let config = FetcherConfig::from_env()?;
    info!("Loaded configuration");
    
    // Create fetcher
    let fetcher = SportsDataIOFetcher::new(config).await?;
    info!("Created fetcher instance");
    
    // Fetch and store season projections
    info!("Fetching and storing season projections...");
    match fetcher.run_season_projections_fetch().await {
        Ok(event) => {
            info!("✅ Successfully stored season projections: {:?}", event);
        }
        Err(e) => {
            error!("❌ Failed to store season projections: {}", e);
            return Err(e);
        }
    }
    
    // Fetch and store player game stats for current week
    let current_week = 4; // TODO: Get from config
    info!("Fetching and storing player game stats for week {}...", current_week);
    match fetcher.run_player_game_stats_fetch(current_week).await {
        Ok(event) => {
            info!("✅ Successfully stored player game stats: {:?}", event);
        }
        Err(e) => {
            error!("❌ Failed to store player game stats: {}", e);
            return Err(e);
        }
    }
    
    info!("Database population completed!");
    Ok(())
}
