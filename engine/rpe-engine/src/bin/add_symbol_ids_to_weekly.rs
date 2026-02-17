use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use tracing::{info, warn};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SeasonProjectionJson {
    player_id: String,
    name: String,
    position: String,
    team: String,
    projected_points: f64,
    symbol_id: u32,
    rank: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SeasonProjectionsJson {
    season: String,
    last_updated: String,
    players: Vec<SeasonProjectionJson>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct WeeklyPlayer {
    player_id: String,
    name: String,
    position: String,
    team: String,
    week: u32,
    fantasy_points: f64,
    opponent: String,
    symbol_id: Option<u32>, // Add this field
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct WeeklyPlayerData {
    season: String,
    week: u32,
    last_updated: String,
    players: Vec<WeeklyPlayer>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    info!("Loading season projections to create symbol_id mapping...");
    
    // Load season projections
    let projections_content = fs::read_to_string("../../data/players/season_projections_2025.json")?;
    let projections: SeasonProjectionsJson = serde_json::from_str(&projections_content)?;
    
    // Create mapping from (name, position, team) to symbol_id
    let mut symbol_id_map: HashMap<(String, String, String), u32> = HashMap::new();
    
    for player in &projections.players {
        let key = (
            player.name.to_lowercase().trim().to_string(),
            player.position.clone(),
            player.team.clone(),
        );
        symbol_id_map.insert(key, player.symbol_id);
    }
    
    info!("Created symbol_id mapping for {} players", symbol_id_map.len());
    
    // Process each weekly stats file
    for week in 1..=5 {
        let filename = format!("../../data/players/week_{}_stats_2025.json", week);
        
        if !std::path::Path::new(&filename).exists() {
            warn!("Weekly stats file not found: {}", filename);
            continue;
        }
        
        info!("Processing week {} stats...", week);
        
        // Load weekly stats
        let weekly_content = fs::read_to_string(&filename)?;
        let mut weekly_data: WeeklyPlayerData = serde_json::from_str(&weekly_content)?;
        
        let mut matched_count = 0;
        let mut unmatched_count = 0;
        
        // Add symbol_id to each player
        for player in &mut weekly_data.players {
            let key = (
                player.name.to_lowercase().trim().to_string(),
                player.position.clone(),
                player.team.clone(),
            );
            
            if let Some(&symbol_id) = symbol_id_map.get(&key) {
                player.symbol_id = Some(symbol_id);
                matched_count += 1;
            } else {
                // Try fuzzy matching for common name variations
                let fuzzy_key = (
                    player.name.to_lowercase().replace(".", "").replace("'", "").trim().to_string(),
                    player.position.clone(),
                    player.team.clone(),
                );
                
                if let Some(&symbol_id) = symbol_id_map.get(&fuzzy_key) {
                    player.symbol_id = Some(symbol_id);
                    matched_count += 1;
                } else {
                    player.symbol_id = None;
                    unmatched_count += 1;
                    if unmatched_count <= 10 { // Only log first 10 unmatched
                        warn!("No symbol_id found for: {} {} {}", player.name, player.position, player.team);
                    }
                }
            }
        }
        
        info!("Week {}: matched {} players, {} unmatched", week, matched_count, unmatched_count);
        
        // Save updated weekly stats
        let updated_content = serde_json::to_string_pretty(&weekly_data)?;
        fs::write(&filename, updated_content)?;
        
        info!("Updated {} with symbol_id fields", filename);
    }
    
    info!("✅ Successfully added symbol_id to all weekly stats files");
    Ok(())
}

