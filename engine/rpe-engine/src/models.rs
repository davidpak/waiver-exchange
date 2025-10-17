use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use bigdecimal::BigDecimal;

/// Season projection data from database
#[derive(Debug, Clone)]
pub struct SeasonProjection {
    pub player_id: i32,
    pub season: i32,
    pub proj_points: BigDecimal,
    pub fantasy_pos: String,
    pub adp: Option<BigDecimal>,
    pub source: String,
    pub ingested_at: DateTime<Utc>,
}

/// Player week points data from database
#[derive(Debug, Clone)]
pub struct PlayerWeekPoints {
    pub player_id: i32,
    pub season: i32,
    pub week: i32,
    pub ts: DateTime<Utc>,
    pub fantasy_pts: BigDecimal,
    pub is_game_over: Option<bool>,
    pub raw: serde_json::Value,
}

/// Calculated fair price record
#[derive(Debug, Clone)]
pub struct FairPriceRecord {
    pub player_id: i32,
    pub ts: DateTime<Utc>,
    pub season: i32,
    pub week: Option<i32>,
    pub fair_cents: i64, // Fₜ
    pub band_bps: i64, // e.g., 3000 = ±30%
    pub kappa_cents_per_pt: i64, // κ
    pub pacing_mode: String, // 'step' | 'poll-step'
    pub actual_pts: BigDecimal,
    pub delta_pts: BigDecimal,
    pub reason: serde_json::Value, // {"projection":true} or {"fp_delta":+6.0}
}

/// RPE calculation result
#[derive(Debug, Clone)]
pub struct RpeCalculation {
    pub player_id: i32,
    pub position: String,
    pub f0_cents: i64, // Pre-game baseline
    pub ft_cents: i64, // Current fair price
    pub actual_pts: BigDecimal,
    pub delta_pts: BigDecimal,
    pub kappa: i64,
    pub reason: CalculationReason,
}

/// Reason for price calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CalculationReason {
    /// Initial projection-based calculation
    Projection {
        season_proj: BigDecimal,
        week_proj: BigDecimal,
    },
    /// Live fantasy points delta
    FantasyPointsDelta {
        delta: BigDecimal,
        current_pts: BigDecimal,
        prev_pts: BigDecimal,
    },
    /// Pacing step (for poll-step mode)
    PacingStep {
        step: u32,
        total_steps: u32,
        target_pts: BigDecimal,
    },
    /// Fair Price 2.0 calculation
    FairPrice2 {
        weeks_played: u8,
        actual_points: f64,
        ema_delta: f64,
        pace: f64,
    },
    /// Fair Price 2.2 - Leaderboard-based calculation
    LeaderboardBased {
        leaderboard_score: f64,
        total_points: f64,
        ppg_shrunk: f64,
        recent_form: f64,
    },
}

impl CalculationReason {
    /// Get weeks played from any calculation reason
    pub fn weeks_played(&self) -> u8 {
        match self {
            CalculationReason::Projection { .. } => 0,
            CalculationReason::FantasyPointsDelta { .. } => 1,
            CalculationReason::PacingStep { step, .. } => *step as u8,
            CalculationReason::FairPrice2 { weeks_played, .. } => *weeks_played,
            CalculationReason::LeaderboardBased { .. } => 6, // Assume current week
        }
    }
}

/// Per-player performance state for Fair Price 2.0
#[derive(Debug, Clone)]
pub struct PlayerPerf {
    pub player_id: i32,
    pub fantasy_pos: String,
    pub proj_points: f64,          // P_proj (season projection)
    pub actual_points: f64,        // Σ FantasyPoints up to current week
    pub weeks_played: u8,          // number of non-bye weeks with data
    pub last_week_points: f64,     // FantasyPoints of the most recent completed week
    pub ema_delta_points: f64,     // EMA over Δpts (recent momentum)
    pub f0_cents: i64,             // initial baseline at t0 (kept for band reference)
    pub fair_cents: i64,           // current F_t
}

/// Events emitted by the RPE
#[derive(Debug, Clone, Serialize)]
pub enum RpeEvent {
    /// Fair price updated
    FairPriceUpdated {
        player_id: i32,
        fair_cents: i64,
        delta_cents: i64,
        reason: CalculationReason,
        timestamp: DateTime<Utc>,
    },
    
    /// Price calculation failed
    CalculationFailed {
        player_id: i32,
        error: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Batch processing completed
    BatchCompleted {
        processed_count: usize,
        updated_count: usize,
        timestamp: DateTime<Utc>,
    },
}

impl FairPriceRecord {
    /// Create a new fair price record
    pub fn new(
        player_id: i32,
        season: i32,
        week: Option<i32>,
        fair_cents: i64,
        band_bps: i64,
        kappa_cents_per_pt: i64,
        pacing_mode: String,
        actual_pts: BigDecimal,
        delta_pts: BigDecimal,
        reason: CalculationReason,
    ) -> Self {
        Self {
            player_id,
            ts: Utc::now(),
            season,
            week,
            fair_cents,
            band_bps,
            kappa_cents_per_pt,
            pacing_mode,
            actual_pts,
            delta_pts,
            reason: serde_json::to_value(reason).unwrap_or_default(),
        }
    }
}

impl RpeCalculation {
    /// Create a new RPE calculation result
    pub fn new(
        player_id: i32,
        position: String,
        f0_cents: i64,
        ft_cents: i64,
        actual_pts: BigDecimal,
        delta_pts: BigDecimal,
        kappa: i64,
        reason: CalculationReason,
    ) -> Self {
        Self {
            player_id,
            position,
            f0_cents,
            ft_cents,
            actual_pts,
            delta_pts,
            kappa,
            reason,
        }
    }
}

impl PlayerPerf {
    /// Create a new PlayerPerf instance
    pub fn new(
        player_id: i32,
        fantasy_pos: String,
        proj_points: f64,
        f0_cents: i64,
    ) -> Self {
        Self {
            player_id,
            fantasy_pos,
            proj_points,
            actual_points: 0.0,
            weeks_played: 0,
            last_week_points: 0.0,
            ema_delta_points: 0.0,
            f0_cents,
            fair_cents: f0_cents,
        }
    }
    
    /// Calculate season pace from actual performance
    pub fn calculate_pace(&self) -> f64 {
        if self.weeks_played > 0 {
            (self.actual_points / self.weeks_played as f64) * 17.0
        } else {
            self.proj_points
        }
    }
    
    /// Update player performance with new week data
    pub fn update_week(&mut self, week_points: f64) {
        let played_this_week = week_points > 0.0;
        
        // Update actual points and weeks played
        self.actual_points += week_points;
        if played_this_week {
            self.weeks_played = self.weeks_played.saturating_add(1);
        }
        
        // Update last week points
        self.last_week_points = week_points;
    }
    
    /// Update EMA delta with new weekly change
    pub fn update_ema_delta(&mut self, smoothing: f64) {
        // For the first week, delta is 0
        if self.weeks_played <= 1 {
            self.ema_delta_points = 0.0;
            return;
        }
        
        // Calculate delta from previous week
        // Since we don't have historical week data, we'll use a simplified approach:
        // delta = current_week - average_so_far (excluding current week)
        let avg_so_far = if self.weeks_played > 1 {
            (self.actual_points - self.last_week_points) / (self.weeks_played - 1) as f64
        } else {
            0.0
        };
        
        let delta = self.last_week_points - avg_so_far;
        
        // Update EMA: EMA_new = γ * delta + (1-γ) * EMA_old
        self.ema_delta_points = smoothing * delta + (1.0 - smoothing) * self.ema_delta_points;
    }
    
    /// Update EMA delta with actual previous week data (for when we have historical data)
    pub fn update_ema_delta_with_previous(&mut self, current_week_points: f64, previous_week_points: f64, smoothing: f64) {
        let delta = current_week_points - previous_week_points;
        
        // Update EMA: EMA_new = γ * delta + (1-γ) * EMA_old
        self.ema_delta_points = smoothing * delta + (1.0 - smoothing) * self.ema_delta_points;
    }
}

// ===== JSON DATA STRUCTURES FOR WEEKLY STATS =====

/// Weekly player data from JSON files
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WeeklyPlayer {
    pub player_id: String,
    pub name: String,
    pub position: String,
    pub team: String,
    pub week: u32,
    pub fantasy_points: f64,
    pub opponent: String,
    pub symbol_id: Option<u32>,
    pub rank: Option<u32>, // Weekly ranking (1 = best)
}

/// Weekly player data container from JSON files
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WeeklyPlayerData {
    pub season: String,
    pub week: u32,
    pub last_updated: DateTime<Utc>,
    pub players: Vec<WeeklyPlayer>,
}

impl WeeklyPlayerData {
    /// Create new weekly data
    pub fn new(season: String, week: u32) -> Self {
        Self {
            season,
            week,
            last_updated: Utc::now(),
            players: Vec::new(),
        }
    }

    /// Sort players by fantasy points (descending)
    pub fn sort_by_points(&mut self) {
        self.players.sort_by(|a, b| b.fantasy_points.partial_cmp(&a.fantasy_points).unwrap());
    }

    /// Get top N players by fantasy points
    pub fn top_players(&self, limit: usize) -> Vec<&WeeklyPlayer> {
        self.players.iter().take(limit).collect()
    }
}

/// Season projection data from JSON files
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SeasonProjectionJson {
    pub player_id: String,
    pub name: String,
    pub position: String,
    pub team: String,
    pub projected_points: f64,
    pub symbol_id: u32,
    pub rank: u32,
}

/// Season projections container from JSON files
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SeasonProjectionsJson {
    pub season: String,
    pub last_updated: DateTime<Utc>,
    pub players: Vec<SeasonProjectionJson>,
}
