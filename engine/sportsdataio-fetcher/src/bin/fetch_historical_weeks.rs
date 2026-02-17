use sportsdataio_fetcher::{
    config::FetcherConfig,
    fetcher::SportsDataIOFetcher,
};
use anyhow::Result;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load configuration
    let config = FetcherConfig::from_env()?;
    let mut fetcher = SportsDataIOFetcher::new(config).await?;
    
    println!("🔄 Fetching Historical Weeks 1-3 Data");
    println!("=====================================");
    
    // Fetch weeks 1-3
    for week in 1..=3 {
        println!("\n📅 Fetching Week {} data...", week);
        
        match fetcher.run_player_game_stats_fetch(week).await {
            Ok(event) => {
                println!("✅ Week {} fetch completed: {:?}", week, event);
            }
            Err(e) => {
                println!("❌ Week {} fetch failed: {}", week, e);
            }
        }
        
        // Small delay between requests
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }
    
    println!("\n🎉 Historical data fetch complete!");
    Ok(())
}
