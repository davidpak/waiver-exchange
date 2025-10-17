use player_scraper::scraper::NflPlayerScraper;
use std::fs;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Create scraper
    let scraper = NflPlayerScraper::new()?;
    
    println!("🏈 NFL Fantasy All Weeks Stats Scraper");
    println!("=====================================");
    
    // Create data directory if it doesn't exist (relative to project root)
    let data_dir = Path::new("../../data/players");
    if !data_dir.exists() {
        fs::create_dir_all(data_dir)?;
        println!("📁 Created data directory: {:?}", data_dir);
    }
    
    // Scrape all weeks 1-6 (comprehensive weekly data)
    let weeks_to_scrape = vec![1, 2, 3, 4, 5, 6];
    
    for week in weeks_to_scrape {
        println!("\n📅 Scraping Week {} data...", week);
        
        match scraper.scrape_weekly_stats("2025", week).await {
            Ok(weekly_data) => {
                // Save to JSON file
                let filename = format!("../../data/players/week_{}_stats_2025.json", week);
                let json_data = serde_json::to_string_pretty(&weekly_data)?;
                fs::write(&filename, json_data)?;
                
                println!("✅ Week {}: Scraped {} players", week, weekly_data.players.len());
                println!("   💾 Saved to: {}", filename);
                
                // Show top 5 players with ranks
                let top_players = weekly_data.top_players(5);
                println!("   🏆 Top 5 players:");
                for player in top_players.iter() {
                    println!("      Rank {}: {} ({}) - {:.1} pts vs {}", 
                        player.rank.unwrap_or(0), player.name, player.position, 
                        player.fantasy_points, player.opponent);
                }
            }
            Err(e) => {
                println!("❌ Week {} failed: {}", week, e);
            }
        }
        
        // Small delay between weeks to be respectful
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }
    
    println!("\n🎉 All weeks scraping complete!");
    println!("📊 Data saved to ../../data/players/ directory");
    
    Ok(())
}
