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
