use player_registry::PlayerRegistry;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Testing PlayerRegistry...");

    // Create registry and load player data
    let mut registry = PlayerRegistry::new();
    registry.load_from_file("data/players/season_projections_2025.json").await?;

    info!("Registry loaded with {} symbols", registry.symbol_count());

    // Test top 10 players
    let top_players = registry.get_top_players(10);
    println!("\nTop 10 Players by Projected Points:");
    println!("Rank Name                 Pos  Team Points   Symbol ID");
    println!("------------------------------------------------------------");

    for (i, player) in top_players.iter().enumerate() {
        println!(
            "{:4} {:20} {:4} {:4} {:8.2} {:10}",
            i + 1,
            player.name,
            player.position,
            player.team,
            player.projected_points,
            player.symbol_id
        );
    }

    // Test symbol lookup
    println!("\nTesting symbol lookups:");

    // Test by name
    if let Ok(lamar) = registry.get_by_name("Lamar Jackson") {
        println!(
            "Found Lamar Jackson: Symbol ID {}, Projected: {:.2}",
            lamar.symbol_id, lamar.projected_points
        );
    }

    // Test by symbol ID
    if let Ok(player) = registry.get_by_symbol_id(0) {
        println!("Symbol ID 0: {} ({})", player.name, player.position);
    }

    // Test search functionality
    println!("\nSearching for 'Jackson':");
    let results = registry.search_players("Jackson");
    for player in results {
        println!("  {} - {} {}", player.name, player.position, player.team);
    }

    // Test symbol name generation
    if let Ok(lamar) = registry.get_by_name("Lamar Jackson") {
        println!("\nSymbol name for Lamar Jackson: {}", lamar.symbol_name);
    }

    info!("PlayerRegistry test completed successfully!");
    Ok(())
}
