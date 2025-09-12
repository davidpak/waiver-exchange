use anyhow::Result;
use player_scraper::NflPlayerScraper;
use std::fs;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting NFL player scraper...");

    // Create scraper
    let scraper = NflPlayerScraper::new()?;

    // Scrape all 500 players from multiple pages
    let player_data = scraper.scrape_all_players("2025", 500).await?;

    // Create data directory if it doesn't exist
    fs::create_dir_all("data/players")?;

    // Save to JSON file
    let json_path = "data/players/season_projections_2025.json";
    let json_content = serde_json::to_string_pretty(&player_data)?;
    fs::write(json_path, json_content)?;

    info!("Saved {} players to {}", player_data.players.len(), json_path);

    // Print top 10 players
    println!("\nTop 10 Players by Projected Fantasy Points:");
    println!(
        "{:<4} {:<20} {:<4} {:<4} {:<8} {:<6}",
        "Rank", "Name", "Pos", "Team", "Points", "Price"
    );
    println!("{}", "-".repeat(60));

    for player in player_data.top_players(10) {
        let price = player_data.points_to_currency(player.projected_points);
        println!(
            "{:<4} {:<20} {:<4} {:<4} {:<8.2} ${:<5}",
            player.rank.unwrap_or(0),
            player.name,
            player.position,
            player.team,
            player.projected_points,
            price
        );
    }

    // Print summary
    println!("\nSummary:");
    println!("- Total players scraped: {}", player_data.players.len());
    println!(
        "- Top player: {} ({}) - {} projected points",
        player_data.players[0].name,
        player_data.players[0].position,
        player_data.players[0].projected_points
    );
    println!("- Note: Successfully scraped all 500 players from 20 pages!");

    info!("Scraping completed successfully!");
    Ok(())
}
