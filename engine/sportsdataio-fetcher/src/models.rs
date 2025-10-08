use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// SportsDataIO season projection data structure
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SeasonProjection {
    #[serde(rename = "PlayerID")]
    pub player_id: i32,
    
    #[serde(rename = "Name")]
    pub name: String,
    
    #[serde(rename = "Position")]
    pub position: String,
    
    #[serde(rename = "Team")]
    pub team: String,
    
    #[serde(rename = "FantasyPoints")]
    pub fantasy_points: Option<f64>,
    
    #[serde(rename = "FantasyPointsPPR")]
    pub fantasy_points_ppr: Option<f64>,
    
    #[serde(rename = "AverageDraftPosition")]
    pub average_draft_position: Option<f64>,
}

/// SportsDataIO player game stats data structure
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlayerGameStats {
    #[serde(rename = "PlayerID")]
    pub player_id: i32,
    
    #[serde(rename = "Name")]
    pub name: String,
    
    #[serde(rename = "Position")]
    pub position: String,
    
    #[serde(rename = "Team")]
    pub team: String,
    
    #[serde(rename = "FantasyPoints")]
    pub fantasy_points: Option<f64>,
    
    #[serde(rename = "FantasyPointsPPR")]
    pub fantasy_points_ppr: Option<f64>,
    
    #[serde(rename = "IsGameOver")]
    pub is_game_over: Option<bool>,
    
    #[serde(rename = "Week")]
    pub week: Option<i32>,
    
    #[serde(rename = "Date")]
    pub date: Option<String>,
}

/// Database model for season projections
#[derive(Debug, Clone)]
pub struct ProjectionRecord {
    pub player_id: i32,
    pub season: i32,
    pub proj_points: f64,
    pub fantasy_pos: String,
    pub adp: Option<f64>,
    pub source: String,
    pub ingested_at: DateTime<Utc>,
}

/// Database model for player week points
#[derive(Debug, Clone)]
pub struct PlayerWeekPointsRecord {
    pub player_id: i32,
    pub season: i32,
    pub week: i32,
    pub ts: DateTime<Utc>,
    pub fantasy_pts: f64,
    pub is_game_over: Option<bool>,
    pub raw: serde_json::Value,
}

/// Events emitted by the fetcher
#[derive(Debug, Clone, Serialize)]
pub enum FetcherEvent {
    /// Season projections updated
    ProjectionsUpdated {
        count: usize,
        timestamp: DateTime<Utc>,
    },
    
    /// Player week points updated
    PlayerWeekPointsUpdated {
        count: usize,
        week: i32,
        timestamp: DateTime<Utc>,
    },
    
    /// Data fetch failed
    FetchFailed {
        endpoint: String,
        error: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Data delayed flag
    DataDelayed {
        reason: String,
        timestamp: DateTime<Utc>,
    },
}

/// API response wrapper
#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub data: Vec<T>,
}

impl SeasonProjection {
    /// Convert to database record
    pub fn to_projection_record(&self, season: i32) -> ProjectionRecord {
        // Use standard FantasyPoints (not PPR)
        let proj_points = self.fantasy_points.unwrap_or(0.0);
        
        ProjectionRecord {
            player_id: self.player_id,
            season,
            proj_points,
            fantasy_pos: self.position.clone(),
            adp: self.average_draft_position,
            source: "sportsdataio".to_string(),
            ingested_at: Utc::now(),
        }
    }
}

impl PlayerGameStats {
    /// Convert to database record
    pub fn to_week_points_record(&self, season: i32, week: i32) -> PlayerWeekPointsRecord {
        // Use ONLY standard FantasyPoints, NEVER PPR
        let fantasy_pts = self.fantasy_points.unwrap_or(0.0);
        
        PlayerWeekPointsRecord {
            player_id: self.player_id,
            season,
            week,
            ts: Utc::now(),
            fantasy_pts,
            is_game_over: self.is_game_over,
            raw: serde_json::to_value(self).unwrap_or_default(),
        }
    }
    
}
