use player_scraper::scraper::{NflPlayerScraper, NflLeaderboardPlayer};
use serde::{Deserialize, Serialize};
use std::fs;


#[derive(Debug, Serialize, Deserialize)]
pub struct NflLeaderboard {
    pub season: u32,
    pub week: u32,
    pub last_updated: String,
    pub players: Vec<NflLeaderboardPlayer>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    println!("🏈 Scraping NFL Fantasy Leaderboard");
    println!("===================================");
    
    let scraper = NflPlayerScraper::new()?;
    let mut all_players = Vec::new();
    let mut current_rank = 1;
    
    // Scrape pages until we have ~500 players (20 pages * 25 players)
    for page in 0..20 { // Up to 500 players (20 pages * 25 players)
        let offset = (page * 25) + 1;
        println!("📄 Scraping page {} (offset: {})", page + 1, offset);
        
        let url = format!(
            "https://fantasy.nfl.com/research/players?offset={}&position=O&sort=pts&statCategory=stats&statSeason=2025&statType=seasonStats&statWeek=7",
            offset
        );
        
        match scraper.scrape_nfl_leaderboard_page(&url).await {
            Ok(mut players) => {
                if players.is_empty() {
                    println!("✅ No more players found, stopping at page {}", page + 1);
                    break;
                }
                
                // Assign ranks
                for player in &mut players {
                    player.rank = current_rank;
                    current_rank += 1;
                }
                
                println!("  Found {} players", players.len());
                
                // Show top 5 players from this page
                println!("  📊 Top 5 from page {}:", page + 1);
                for (i, player) in players.iter().take(5).enumerate() {
                    println!("    {}. {} ({}) - {:.2} pts", 
                             i + 1, player.name, player.position, player.fantasy_points);
                }
                
                all_players.extend(players);
                
                // Stop when we have enough players (around 500)
                if all_players.len() >= 500 {
                    println!("✅ Reached target player count ({}), stopping", all_players.len());
                    break;
                }
            }
            Err(e) => {
                println!("❌ Error scraping page {}: {}", page + 1, e);
                break;
            }
        }
        
        // Small delay to be respectful
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    
    println!("📊 Total players scraped: {}", all_players.len());
    
    // Create leaderboard data
    let leaderboard = NflLeaderboard {
        season: 2025,
        week: 7,
        last_updated: chrono::Utc::now().to_rfc3339(),
        players: all_players,
    };
    
    // Save to file
    let filename = "data/players/nfl_fantasy_leaderboard_2025.json";
    let json = serde_json::to_string_pretty(&leaderboard)?;
    fs::write(filename, json)?;
    
    println!("💾 Saved leaderboard to {}", filename);
    
    // Show top 10
    println!("\n🏆 Top 10 Players:");
    for (i, player) in leaderboard.players.iter().take(10).enumerate() {
        println!("  {}. {} ({}) - {:.2} pts", 
                 i + 1, player.name, player.position, player.fantasy_points);
    }
    
    Ok(())
}
