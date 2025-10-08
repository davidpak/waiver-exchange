use sportsdataio_fetcher::{FetcherConfig, SportsDataIOFetcher};
use tracing::{info, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Debugging SportsDataIO data for Ja'Marr Chase");
    
    // Load configuration
    let config = FetcherConfig::from_env()?;
    info!("Loaded configuration");
    
    // Create fetcher
    let fetcher = SportsDataIOFetcher::new(config).await?;
    info!("Created fetcher instance");
    
    // Fetch season projections
    info!("Fetching season projections...");
    let projections = fetcher.fetch_season_projections().await?;
    
    // Find Ja'Marr Chase (PlayerID: 22564)
    if let Some(chase) = projections.iter().find(|p| p.player_id == 22564) {
        info!("Found Ja'Marr Chase:");
        info!("  PlayerID: {}", chase.player_id);
        info!("  Name: {}", chase.name);
        info!("  Position: {}", chase.position);
        info!("  Team: {}", chase.team);
        info!("  FantasyPoints: {:?}", chase.fantasy_points);
        info!("  FantasyPointsPPR: {:?}", chase.fantasy_points_ppr);
        info!("  AverageDraftPosition: {:?}", chase.average_draft_position);
        
        // Show what we would store
        let record = chase.to_projection_record(2025);
        info!("  Would store proj_points: {}", record.proj_points);
    } else {
        error!("Ja'Marr Chase not found in projections!");
    }
    
    // Fetch player game stats for week 4
    info!("Fetching player game stats for week 4...");
    let stats = fetcher.fetch_player_game_stats(4).await?;
    
    // Find Ja'Marr Chase in week 4 stats
    if let Some(chase) = stats.iter().find(|p| p.player_id == 22564) {
        info!("Found Ja'Marr Chase in week 4 stats:");
        info!("  PlayerID: {}", chase.player_id);
        info!("  Name: {}", chase.name);
        info!("  Position: {}", chase.position);
        info!("  Team: {}", chase.team);
        info!("  FantasyPoints: {:?}", chase.fantasy_points);
        info!("  FantasyPointsPPR: {:?}", chase.fantasy_points_ppr);
        info!("  IsGameOver: {:?}", chase.is_game_over);
        
        // Show what we would store
        let record = chase.to_week_points_record(2025, 4);
        info!("  Would store fantasy_pts: {}", record.fantasy_pts);
    } else {
        error!("Ja'Marr Chase not found in week 4 stats!");
    }
    
    info!("Debug completed!");
    Ok(())
}
