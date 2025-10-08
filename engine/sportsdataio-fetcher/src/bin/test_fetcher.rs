use sportsdataio_fetcher::{FetcherConfig, SportsDataIOFetcher};
use tracing::{info, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Testing SportsDataIO Fetcher");
    
    // Load configuration
    let config = FetcherConfig::from_env()?;
    info!("Loaded configuration");
    
    // Create fetcher
    let fetcher = SportsDataIOFetcher::new(config).await?;
    info!("Created fetcher instance");
    
    // Test fetching season projections
    info!("Testing season projections fetch...");
    match fetcher.fetch_season_projections().await {
        Ok(projections) => {
            info!("✅ Successfully fetched {} season projections", projections.len());
            
            // Show first few projections
            for (i, proj) in projections.iter().take(3).enumerate() {
                info!("  {}. {} ({}) - {} pts", 
                      i + 1, 
                      proj.name, 
                      proj.position, 
                      proj.fantasy_points.unwrap_or(0.0)
                );
            }
        }
        Err(e) => {
            error!("❌ Failed to fetch season projections: {}", e);
        }
    }
    
    // Test fetching player game stats for current week
    let current_week = fetcher.get_current_week();
    info!("Testing player game stats fetch for week {}...", current_week);
    match fetcher.fetch_player_game_stats(current_week).await {
        Ok(stats) => {
            info!("✅ Successfully fetched {} player game stats for week {}", stats.len(), current_week);
            
            // Show first few stats
            for (i, stat) in stats.iter().take(3).enumerate() {
                info!("  {}. {} ({}) - {} pts", 
                      i + 1, 
                      stat.name, 
                      stat.position, 
                      stat.fantasy_points.unwrap_or(0.0)
                );
            }
        }
        Err(e) => {
            error!("❌ Failed to fetch player game stats: {}", e);
        }
    }
    
    info!("Test completed!");
    Ok(())
}
