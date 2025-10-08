use crate::config::RpeConfig;
use crate::models::*;
use anyhow::Result;
use bigdecimal::{BigDecimal, ToPrimitive};
use tracing::info;

/// Price calculator for RPE calculations
pub struct PriceCalculator {
    config: RpeConfig,
}

impl PriceCalculator {
    /// Create a new price calculator
    pub fn new(config: RpeConfig) -> Self {
        Self { config }
    }
    
    /// Calculate F₀ (pre-game baseline price) from season projection
    pub fn calculate_f0(&self, projection: &SeasonProjection) -> Result<i64> {
        let season_proj = projection.proj_points.to_f64().unwrap_or(0.0);
        let week_proj = season_proj / 17.0; // P_week = P_season / 17
        
        // F₀ = base + β_season * (P_season / 17) 
        // Removed double-counting of week projection - season projection is the primary driver
        let f0_cents = self.config.rpe.base_cents
            + (self.config.rpe.beta_season_cents_per_pt as f64 * (season_proj / 17.0)) as i64;
        
        info!(
            "Calculated F₀ for player {}: {} cents (season_proj: {:.2}, week_proj: {:.2})",
            projection.player_id, f0_cents, season_proj, week_proj
        );
        
        Ok(f0_cents)
    }
    
    /// Calculate Fₜ (current fair price) with live fantasy points delta
    pub fn calculate_ft_with_delta(
        &self,
        player_id: i32,
        position: &str,
        f0_cents: i64,
        current_pts: BigDecimal,
        prev_pts: BigDecimal,
    ) -> Result<i64> {
        let delta_pts = current_pts.clone() - prev_pts.clone();
        let delta_pts_f64 = delta_pts.to_f64().unwrap_or(0.0);
        
        let kappa = self.config.get_kappa_for_position(position);
        
        // Fₜ = clip(F_(t-1) + κ * Δpts, band)
        let ft_cents = f0_cents + (kappa as f64 * delta_pts_f64) as i64;
        
        // Apply band clipping (±30% by default)
        let band_factor = self.config.rpe.ingame_band_bps as f64 / 10000.0; // Convert bps to decimal
        let min_price = (f0_cents as f64 * (1.0 - band_factor)) as i64;
        let max_price = (f0_cents as f64 * (1.0 + band_factor)) as i64;
        
        let clipped_ft = ft_cents.max(min_price).min(max_price);
        
        info!(
            "Calculated Fₜ for player {}: {} cents (delta: {:.2}, kappa: {}, clipped: {})",
            player_id, clipped_ft, delta_pts_f64, kappa, clipped_ft != ft_cents
        );
        
        Ok(clipped_ft)
    }
    
    /// Calculate Fₜ with pacing (for poll-step mode)
    pub fn calculate_ft_with_pacing(
        &self,
        player_id: i32,
        position: &str,
        f0_cents: i64,
        current_pts: BigDecimal,
        week_proj: BigDecimal,
        step: u32,
    ) -> Result<i64> {
        let current_pts_f64 = current_pts.to_f64().unwrap_or(0.0);
        let week_proj_f64 = week_proj.to_f64().unwrap_or(0.0);
        
        let kappa = self.config.get_kappa_for_position(position);
        
        // F_drift_target = F₀ + κ * (FantasyPoints_now − P_week * step/N)
        let target_pts = week_proj_f64 * (step as f64 / self.config.rpe.pacing_steps as f64);
        let pts_delta = current_pts_f64 - target_pts;
        
        let ft_cents = f0_cents + (kappa as f64 * pts_delta) as i64;
        
        // Apply band clipping
        let band_factor = self.config.rpe.ingame_band_bps as f64 / 10000.0;
        let min_price = (f0_cents as f64 * (1.0 - band_factor)) as i64;
        let max_price = (f0_cents as f64 * (1.0 + band_factor)) as i64;
        
        let clipped_ft = ft_cents.max(min_price).min(max_price);
        
        info!(
            "Calculated Fₜ with pacing for player {}: {} cents (step: {}/{}, target_pts: {:.2})",
            player_id, clipped_ft, step, self.config.rpe.pacing_steps, target_pts
        );
        
        Ok(clipped_ft)
    }
    
    /// Calculate RPE result from season projection
    pub fn calculate_from_projection(&self, projection: &SeasonProjection) -> Result<RpeCalculation> {
        let f0_cents = self.calculate_f0(projection)?;
        let week_proj = projection.proj_points.clone() / BigDecimal::from(17);
        
        let reason = CalculationReason::Projection {
            season_proj: projection.proj_points.clone(),
            week_proj: week_proj.clone(),
        };
        
        Ok(RpeCalculation::new(
            projection.player_id,
            projection.fantasy_pos.clone(),
            f0_cents,
            f0_cents, // Fₜ = F₀ initially
            projection.proj_points.clone(),
            BigDecimal::from(0), // No delta initially
            self.config.get_kappa_for_position(&projection.fantasy_pos),
            reason,
        ))
    }
    
    /// Calculate RPE result from fantasy points delta
    pub fn calculate_from_delta(
        &self,
        player_id: i32,
        position: &str,
        f0_cents: i64,
        current_pts: BigDecimal,
        prev_pts: BigDecimal,
    ) -> Result<RpeCalculation> {
        let ft_cents = self.calculate_ft_with_delta(player_id, position, f0_cents, current_pts.clone(), prev_pts.clone())?;
        let delta_pts = current_pts.clone() - prev_pts.clone();
        
        let reason = CalculationReason::FantasyPointsDelta {
            delta: delta_pts.clone(),
            current_pts: current_pts.clone(),
            prev_pts: prev_pts.clone(),
        };
        
        Ok(RpeCalculation::new(
            player_id,
            position.to_string(),
            f0_cents,
            ft_cents,
            current_pts,
            delta_pts,
            self.config.get_kappa_for_position(position),
            reason,
        ))
    }
    
    /// Calculate RPE result with pacing
    pub fn calculate_with_pacing(
        &self,
        player_id: i32,
        position: &str,
        f0_cents: i64,
        current_pts: BigDecimal,
        week_proj: BigDecimal,
        step: u32,
    ) -> Result<RpeCalculation> {
        let ft_cents = self.calculate_ft_with_pacing(player_id, position, f0_cents, current_pts.clone(), week_proj.clone(), step)?;
        
        let reason = CalculationReason::PacingStep {
            step,
            total_steps: self.config.rpe.pacing_steps,
            target_pts: week_proj.clone() * BigDecimal::from(step) / BigDecimal::from(self.config.rpe.pacing_steps),
        };
        
        Ok(RpeCalculation::new(
            player_id,
            position.to_string(),
            f0_cents,
            ft_cents,
            current_pts,
            BigDecimal::from(0), // No direct delta in pacing mode
            self.config.get_kappa_for_position(position),
            reason,
        ))
    }
}
