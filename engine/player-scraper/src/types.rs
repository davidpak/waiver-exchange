use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a fantasy football player with their season projections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    /// NFL.com player ID
    pub player_id: String,
    /// Player name (e.g., "Josh Allen")
    pub name: String,
    /// Position (QB, RB, WR, TE, K, DEF)
    pub position: String,
    /// Team abbreviation (e.g., "BUF")
    pub team: String,
    /// Projected fantasy points for the season
    pub projected_points: f64,
    /// Symbol ID for trading (assigned after scraping)
    pub symbol_id: Option<u32>,
    /// Rank based on projected points
    pub rank: Option<u32>,
}

/// Container for all scraped player data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerData {
    /// Season year
    pub season: String,
    /// When this data was last updated
    pub last_updated: DateTime<Utc>,
    /// List of players with their projections
    pub players: Vec<Player>,
}

impl PlayerData {
    /// Create new player data container
    pub fn new(season: String) -> Self {
        Self { season, last_updated: Utc::now(), players: Vec::new() }
    }

    /// Sort players by projected points (descending) and assign ranks
    pub fn sort_and_rank(&mut self) {
        self.players.sort_by(|a, b| b.projected_points.partial_cmp(&a.projected_points).unwrap());

        for (index, player) in self.players.iter_mut().enumerate() {
            player.rank = Some((index + 1) as u32);
        }
    }

    /// Get top N players by projected points
    pub fn top_players(&self, limit: usize) -> Vec<&Player> {
        self.players.iter().take(limit).collect()
    }

    /// Convert fantasy points to currency (1 point = $10)
    pub fn points_to_currency(&self, points: f64) -> u32 {
        (points * 10.0) as u32
    }
}
