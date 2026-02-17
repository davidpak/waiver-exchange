use anyhow::{Context, Result};
use fuzzy_matcher::FuzzyMatcher;
use player_registry::PlayerRegistry;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{info, warn};

/// SportsDataIO player projection data structure
#[derive(Debug, Deserialize, Serialize)]
struct SportsDataIOPlayer {
    #[serde(rename = "PlayerID")]
    player_id: i32,
    
    #[serde(rename = "Name")]
    name: String,
    
    #[serde(rename = "Position")]
    position: String,
    
    #[serde(rename = "Team")]
    team: String,
    
    #[serde(rename = "FantasyPoints")]
    fantasy_points: Option<f64>,
    
    #[serde(rename = "FantasyPointsPPR")]
    fantasy_points_ppr: Option<f64>,
    
    #[serde(rename = "AverageDraftPosition")]
    average_draft_position: Option<f64>,
}

/// Our internal player mapping record
#[derive(Debug)]
struct PlayerMapping {
    sportsdataio_player_id: i32,
    our_symbol_id: u32,
    player_name: String,
    team: String,
    position: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("🚀 Starting Player ID Mapping Script");
    info!("🚀 Starting Player ID Mapping Script");
    
    // Get database URL from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/waiver_exchange".to_string());
    
    println!("🔌 Attempting to connect to database...");
    println!("📡 Database URL: {}", database_url);
    
    // Create database connection with timeout
    let connect_future = PgPool::connect(&database_url);
    let timeout_future = tokio::time::timeout(std::time::Duration::from_secs(10), connect_future);
    
    let pool = timeout_future.await
        .context("Database connection timed out after 10 seconds")?
        .context("Failed to connect to database")?;
    
    println!("✅ Database connection successful!");
    
    // Load our existing player registry
    let mut registry = PlayerRegistry::new();
    
    // Try multiple possible paths for the player data file
    let possible_paths = [
        "data/players/season_projections_2025.json",
        "../data/players/season_projections_2025.json",
        "../../data/players/season_projections_2025.json",
    ];
    
    let mut loaded = false;
    for path in &possible_paths {
        println!("🔍 Trying to load player data from: {}", path);
        if let Ok(_) = registry.load_from_file(path).await {
            println!("✅ Successfully loaded player data from: {}", path);
            loaded = true;
            break;
        }
    }
    
    if !loaded {
        anyhow::bail!("Failed to load player registry from any of the attempted paths: {:?}", possible_paths);
    }
    
    println!("📊 Loaded {} players from registry", registry.symbol_count());
    
    // Fetch SportsDataIO projections
    println!("🌐 Fetching SportsDataIO projections...");
    let sportsdataio_players = fetch_sportsdataio_projections().await
        .context("Failed to fetch SportsDataIO projections")?;
    
    println!("📡 Fetched {} players from SportsDataIO", sportsdataio_players.len());
    
    // Create mappings
    println!("🔗 Creating player mappings...");
    let mappings = create_player_mappings(&registry, &sportsdataio_players);
    
    println!("🔗 Created {} player mappings", mappings.len());
    
    // Store mappings in database
    println!("💾 Storing mappings in database...");
    store_mappings(&pool, &mappings).await
        .context("Failed to store mappings in database")?;
    
    println!("🎉 Successfully completed player ID mapping!");
    
    Ok(())
}

async fn fetch_sportsdataio_projections() -> Result<Vec<SportsDataIOPlayer>> {
    let url = "https://api.sportsdata.io/v3/nfl/projections/json/PlayerSeasonProjectionStats/2025?key=2d60a5317f014813810755b281f8c2ea";
    
    println!("🌐 Fetching data from: {}", url);
    
    // Create HTTP client with timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("Failed to create HTTP client")?;
    
    println!("⏱️  Making HTTP request with 30s timeout...");
    let response = client.get(url).send().await
        .context("Failed to fetch from SportsDataIO API")?;
    
    println!("📡 Received response with status: {}", response.status());
    
    if !response.status().is_success() {
        anyhow::bail!("API request failed with status: {}", response.status());
    }
    
    println!("📄 Parsing JSON response...");
    let players: Vec<SportsDataIOPlayer> = response.json().await
        .context("Failed to parse JSON response")?;
    
    println!("✅ Successfully parsed {} players from JSON", players.len());
    
    Ok(players)
}

fn create_player_mappings(
    registry: &PlayerRegistry,
    sportsdataio_players: &[SportsDataIOPlayer],
) -> Vec<PlayerMapping> {
    let mut mappings = Vec::new();
    let mut unmatched_sportsdataio = Vec::new();
    
    println!("📋 Processing {} SportsDataIO players against {} registry players", 
             sportsdataio_players.len(), registry.symbol_count());
    
    // Create a map of our players by name for faster lookup
    let our_players: HashMap<String, (u32, String, String)> = registry
        .get_all_symbols()
        .iter()
        .map(|symbol| {
            (symbol.name.clone(), (symbol.symbol_id, symbol.team.clone(), symbol.position.clone()))
        })
        .collect();
    
    println!("🔍 Starting exact name matching...");
    
    let mut exact_matches = 0;
    let mut fuzzy_matches = 0;
    
    for sio_player in sportsdataio_players {
        // Try exact name match first
        if let Some((symbol_id, team, position)) = our_players.get(&sio_player.name) {
            mappings.push(PlayerMapping {
                sportsdataio_player_id: sio_player.player_id,
                our_symbol_id: *symbol_id,
                player_name: sio_player.name.clone(),
                team: team.clone(),
                position: position.clone(),
            });
            exact_matches += 1;
            continue;
        }
        
        // Try fuzzy matching for name variations
        let mut best_match = None;
        let mut best_score = 0;
        
        for (our_name, (symbol_id, team, position)) in &our_players {
            // Check if positions match first
            if normalize_position(&sio_player.position) != normalize_position(position) {
                continue;
            }
            
            // Try fuzzy matching on names
            let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
            let score = matcher.fuzzy_match(our_name, &sio_player.name)
                .unwrap_or(0);
            
            if score > best_score && score > 60 { // Threshold for fuzzy matching
                best_match = Some((*symbol_id, team.clone(), position.clone()));
                best_score = score;
            }
        }
        
        if let Some((symbol_id, team, position)) = best_match {
            println!("🔍 Fuzzy matched '{}' -> '{}' (score: {})", 
                  sio_player.name, 
                  our_players.iter().find(|(_, (id, _, _))| *id == symbol_id).unwrap().0,
                  best_score);
            
            mappings.push(PlayerMapping {
                sportsdataio_player_id: sio_player.player_id,
                our_symbol_id: symbol_id,
                player_name: sio_player.name.clone(),
                team,
                position,
            });
            fuzzy_matches += 1;
        } else {
            unmatched_sportsdataio.push(sio_player);
        }
    }
    
    println!("✅ Exact matches: {}", exact_matches);
    println!("🔍 Fuzzy matches: {}", fuzzy_matches);
    println!("❌ Unmatched players: {}", unmatched_sportsdataio.len());
    
    // Log unmatched players for manual review
    if !unmatched_sportsdataio.is_empty() {
        warn!("⚠️  {} players could not be matched:", unmatched_sportsdataio.len());
        for player in &unmatched_sportsdataio {
            warn!("   - {} ({}, {})", player.name, player.position, player.team);
        }
    }
    
    mappings
}

fn normalize_position(pos: &str) -> String {
    pos.to_uppercase()
}

async fn store_mappings(pool: &PgPool, mappings: &[PlayerMapping]) -> Result<()> {
    println!("💾 Storing {} mappings in database", mappings.len());
    
    // Clear existing mappings first
    println!("🗑️  Clearing existing mappings...");
    sqlx::query("DELETE FROM player_id_mapping")
        .execute(pool)
        .await
        .context("Failed to clear existing mappings")?;
    println!("✅ Cleared existing mappings");
    
    // Insert new mappings
    println!("📝 Inserting {} new mappings...", mappings.len());
    for (i, mapping) in mappings.iter().enumerate() {
        if i % 50 == 0 {
            println!("   Progress: {}/{}", i, mappings.len());
        }
        
        sqlx::query(
            r#"
            INSERT INTO player_id_mapping 
            (sportsdataio_player_id, our_symbol_id, player_name, team, position)
            VALUES ($1, $2, $3, $4, $5)
            "#
        )
        .bind(mapping.sportsdataio_player_id)
        .bind(mapping.our_symbol_id as i32)
        .bind(&mapping.player_name)
        .bind(&mapping.team)
        .bind(&mapping.position)
        .execute(pool)
        .await
        .with_context(|| format!("Failed to insert mapping for player: {}", mapping.player_name))?;
    }
    println!("✅ Inserted all mappings");
    
    // Verify the insertions
    println!("🔍 Verifying insertions...");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM player_id_mapping")
        .fetch_one(pool)
        .await
        .context("Failed to count mappings")?;
    
    println!("✅ Successfully stored {} mappings in database", count);
    
    Ok(())
}
