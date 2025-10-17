use crate::config::RpeConfig;
use crate::models::*;
use anyhow::{Result, Context};
use bigdecimal::{BigDecimal, ToPrimitive, FromPrimitive};
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
    
    // ===== FAIR PRICE 2.0 METHODS =====
    
    /// Calculate Fair Price 2.0 using adaptive blending and EMA momentum
    pub fn calculate_fair2(
        &self,
        player_perf: &mut PlayerPerf,
        week_points: f64,
    ) -> Result<i64> {
        let fair2_config = self.config.get_fair2_config()
            .ok_or_else(|| anyhow::anyhow!("Fair Price 2.0 not enabled"))?;
        
        // 1) Update player performance with new week data
        player_perf.update_week(week_points);
        
        // 2) Calculate momentum EMA
        player_perf.update_ema_delta(fair2_config.ema_delta.smoothing);
        
        // 3) Calculate blended projection
        let p_pace = player_perf.calculate_pace();
        let alpha = fair2_config.calculate_alpha(player_perf.weeks_played);
        let p_blend = alpha * player_perf.proj_points + (1.0 - alpha) * p_pace;
        
        // 4) Calculate base price from blended projection
        let f_base = fair2_config.base_cents as f64 
            + fair2_config.beta_cents_per_pt as f64 * (p_blend / 17.0);
        
        // 5) Calculate momentum component
        let kappa = fair2_config.get_kappa_for_position(&player_perf.fantasy_pos) as f64;
        let f_mom = kappa * player_perf.ema_delta_points;
        
        // 6) Calculate final price before band clipping
        let f_star = f_base + f_mom;
        
        // 7) Apply band clipping vs F₀
        let band = fair2_config.band_bps as f64 / 10000.0;
        let lower = player_perf.f0_cents as f64 * (1.0 - band);
        let upper = player_perf.f0_cents as f64 * (1.0 + band);
        let f_new = f_star.clamp(lower, upper).round() as i64;
        
        // 8) Update player performance
        player_perf.fair_cents = f_new;
        
        info!(
            "Fair2 calculation for player {}: {} cents (alpha: {:.3}, blend: {:.1}, pace: {:.1}, mom: {:.1})",
            player_perf.player_id, f_new, alpha, p_blend, p_pace, f_mom
        );
        
        Ok(f_new)
    }
    
    /// Calculate Fair Price 2.0 with consistency adjustment
    pub fn calculate_fair2_with_consistency(
        &self,
        player_perf: &mut PlayerPerf,
        week_points: f64,
        weekly_points_history: &[f64], // Last few weeks of points for volatility calculation
    ) -> Result<i64> {
        let fair2_config = self.config.get_fair2_config()
            .ok_or_else(|| anyhow::anyhow!("Fair Price 2.0 not enabled"))?;
        
        // 1) Update player performance with new week data
        player_perf.update_week(week_points);
        
        // 2) Calculate momentum EMA
        player_perf.update_ema_delta(fair2_config.ema_delta.smoothing);
        
        // 3) Calculate blended projection
        let p_pace = player_perf.calculate_pace();
        let alpha = fair2_config.calculate_alpha(player_perf.weeks_played);
        let p_blend = alpha * player_perf.proj_points + (1.0 - alpha) * p_pace;
        
        // 4) Calculate base price from blended projection
        let f_base = fair2_config.base_cents as f64 
            + fair2_config.beta_cents_per_pt as f64 * (p_blend / 17.0);
        
        // 5) Calculate momentum component with consistency adjustment
        let mut kappa = fair2_config.get_kappa_for_position(&player_perf.fantasy_pos) as f64;
        
        if fair2_config.consistency.enabled 
            && player_perf.weeks_played >= fair2_config.consistency.min_weeks_for_sigma as u8
            && weekly_points_history.len() >= fair2_config.consistency.min_weeks_for_sigma as usize {
            
            // Calculate rolling standard deviation
            let sigma = self.calculate_rolling_stddev(weekly_points_history);
            
            // Adjust kappa based on volatility
            kappa = kappa / (1.0 + sigma / fair2_config.consistency.scale);
        }
        
        let f_mom = kappa * player_perf.ema_delta_points;
        
        // 6) Calculate final price before band clipping
        let f_star = f_base + f_mom;
        
        // 7) Apply band clipping vs F₀
        let band = fair2_config.band_bps as f64 / 10000.0;
        let lower = player_perf.f0_cents as f64 * (1.0 - band);
        let upper = player_perf.f0_cents as f64 * (1.0 + band);
        let f_new = f_star.clamp(lower, upper).round() as i64;
        
        // 8) Update player performance
        player_perf.fair_cents = f_new;
        
        info!(
            "Fair2 with consistency for player {}: {} cents (alpha: {:.3}, blend: {:.1}, pace: {:.1}, mom: {:.1}, kappa_adj: {:.1})",
            player_perf.player_id, f_new, alpha, p_blend, p_pace, f_mom, kappa
        );
        
        Ok(f_new)
    }
    
    /// Calculate rolling standard deviation for consistency adjustment
    fn calculate_rolling_stddev(&self, points: &[f64]) -> f64 {
        if points.len() < 2 {
            return 0.0;
        }
        
        let mean = points.iter().sum::<f64>() / points.len() as f64;
        let variance = points.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / (points.len() - 1) as f64;
        
        variance.sqrt()
    }
    
    /// Initialize PlayerPerf from season projection
    pub fn initialize_player_perf(&self, projection: &SeasonProjection) -> Result<PlayerPerf> {
        let f0_cents = self.calculate_f0(projection)?;
        let proj_points = projection.proj_points.to_f64().unwrap_or(0.0);
        
        Ok(PlayerPerf::new(
            projection.player_id,
            projection.fantasy_pos.clone(),
            proj_points,
            f0_cents,
        ))
    }

    /// Initialize PlayerPerf from JSON projection data
    pub fn initialize_player_perf_from_json(&self, projection: &SeasonProjectionJson) -> Result<PlayerPerf> {
        let fair2_config = self.config.get_fair2_config()
            .context("Fair Price 2.0 not configured")?;
        
        let season_proj = projection.projected_points;
        let f0_cents = fair2_config.base_cents + 
            (fair2_config.beta_cents_per_pt as f64 * (season_proj / 17.0)) as i64;
        
        Ok(PlayerPerf::new(
            projection.player_id.parse::<i32>().unwrap_or_default(),
            projection.position.clone(),
            season_proj,
            f0_cents,
        ))
    }

    /// Calculate Fair Price 2.0 from weekly data
    pub fn calculate_fair2_from_weekly_data(
        &self,
        projection: &SeasonProjectionJson,
        weekly_data: &[(u32, f64)],
    ) -> Result<RpeCalculation> {
        // Initialize player performance from JSON projection
        let mut player_perf = self.initialize_player_perf_from_json(projection)?;
        
        // Process weekly data in chronological order
        let mut sorted_weekly_data = weekly_data.to_vec();
        sorted_weekly_data.sort_by_key(|(week, _)| *week);
        
        // Calculate Fair Price 2.0 for each week
        for (_week, points) in sorted_weekly_data {
            let _fair_price = self.calculate_fair2(&mut player_perf, points)?;
        }
        
        // Create calculation result
        let calculation = RpeCalculation {
            player_id: projection.player_id.parse::<i32>().unwrap_or_default(),
            position: projection.position.clone(),
            f0_cents: player_perf.f0_cents,
            ft_cents: player_perf.fair_cents,
            actual_pts: BigDecimal::from_f64(player_perf.actual_points).unwrap_or_default(),
            delta_pts: BigDecimal::from_f64(player_perf.ema_delta_points).unwrap_or_default(),
            kappa: self.config.get_fair2_config()
                .map(|cfg| cfg.get_kappa_for_position(&projection.position))
                .unwrap_or(150),
            reason: CalculationReason::FairPrice2 {
                weeks_played: player_perf.weeks_played,
                actual_points: player_perf.actual_points,
                ema_delta: player_perf.ema_delta_points,
                pace: player_perf.calculate_pace(),
            },
        };
        
        Ok(calculation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;
    use chrono::Utc;

    fn create_test_config() -> RpeConfig {
        RpeConfig::default()
    }

    fn create_test_projection(player_id: i32, proj_points: f64, position: &str) -> SeasonProjection {
        SeasonProjection {
            player_id,
            season: 2025,
            proj_points: BigDecimal::from(proj_points as i64), // Convert to integer first
            fantasy_pos: position.to_string(),
            adp: Some(BigDecimal::from(50)),
            source: "test".to_string(),
            ingested_at: Utc::now(),
        }
    }


    #[test]
    fn test_fair2_initialization() {
        let config = create_test_config();
        let calculator = PriceCalculator::new(config);
        let projection = create_test_projection(123, 200.0, "RB");
        
        let player_perf = calculator.initialize_player_perf(&projection).unwrap();
        
        assert_eq!(player_perf.player_id, 123);
        assert_eq!(player_perf.fantasy_pos, "RB");
        assert_eq!(player_perf.proj_points, 200.0);
        assert_eq!(player_perf.actual_points, 0.0);
        assert_eq!(player_perf.weeks_played, 0);
        assert_eq!(player_perf.fair_cents, player_perf.f0_cents);
    }

    #[test]
    fn test_fair2_alpha_blending() {
        let config = create_test_config();
        let calculator = PriceCalculator::new(config);
        let projection = create_test_projection(123, 200.0, "RB");
        
        let mut player_perf = calculator.initialize_player_perf(&projection).unwrap();
        
        // Week 1: Should be mostly projection-based
        let price1 = calculator.calculate_fair2(&mut player_perf, 15.0).unwrap();
        assert!(price1 > 0);
        
        // Week 8: Should be more balanced
        for week in 2..=8 {
            let week_points = 12.0 + (week as f64 * 0.5); // Slightly increasing performance
            let _price = calculator.calculate_fair2(&mut player_perf, week_points).unwrap();
        }
        
        // Week 17: Should be mostly performance-based
        for week in 9..=17 {
            let week_points = 12.0 + (week as f64 * 0.5);
            let _price = calculator.calculate_fair2(&mut player_perf, week_points).unwrap();
        }
        
        // Final price should reflect the actual performance pace
        assert!(player_perf.weeks_played == 17);
        assert!(player_perf.actual_points > 0.0);
    }

    #[test]
    fn test_fair2_ema_momentum() {
        let config = create_test_config();
        let calculator = PriceCalculator::new(config);
        let projection = create_test_projection(123, 200.0, "RB");
        
        let mut player_perf = calculator.initialize_player_perf(&projection).unwrap();
        
        // Consistent performance should have low momentum
        for _week in 1..=5 {
            let _price = calculator.calculate_fair2(&mut player_perf, 12.0).unwrap();
        }
        
        let consistent_ema = player_perf.ema_delta_points;
        
        // Reset and test breakout performance
        let mut breakout_perf = calculator.initialize_player_perf(&projection).unwrap();
        
        // Breakout performance should have high momentum
        for week in 1..=5 {
            let week_points = if week <= 3 { 8.0 } else { 25.0 }; // Breakout in week 4-5
            let _price = calculator.calculate_fair2(&mut breakout_perf, week_points).unwrap();
        }
        
        let breakout_ema = breakout_perf.ema_delta_points;
        
        println!("Consistent EMA: {}, Breakout EMA: {}", consistent_ema, breakout_ema);
        
        // Breakout should have higher momentum
        assert!(breakout_ema > consistent_ema);
    }

    #[test]
    fn test_fair2_band_clipping() {
        let config = create_test_config();
        let calculator = PriceCalculator::new(config);
        let projection = create_test_projection(123, 200.0, "RB");
        
        let mut player_perf = calculator.initialize_player_perf(&projection).unwrap();
        let f0_cents = player_perf.f0_cents;
        
        // Test with multiple extreme performances to ensure clipping works
        for _week in 1..=5 {
            let _price = calculator.calculate_fair2(&mut player_perf, 50.0).unwrap();
        }
        
        let final_price = player_perf.fair_cents;
        
        // Price should be within ±30% of F₀ (with small tolerance for rounding)
        let band = 0.30;
        let min_price = (f0_cents as f64 * (1.0 - band)) as i64;
        let max_price = (f0_cents as f64 * (1.0 + band)) as i64;
        
        println!("F0: {}, Final: {}, Min: {}, Max: {}", f0_cents, final_price, min_price, max_price);
        
        assert!(final_price >= min_price);
        assert!(final_price <= max_price + 1); // Allow 1 cent tolerance for rounding
    }

    #[test]
    fn test_fair2_consistency_adjustment() {
        let config = create_test_config();
        let calculator = PriceCalculator::new(config);
        let projection = create_test_projection(123, 200.0, "RB");
        
        let mut consistent_perf = calculator.initialize_player_perf(&projection).unwrap();
        let mut volatile_perf = calculator.initialize_player_perf(&projection).unwrap();
        
        // Consistent performance (low volatility)
        let consistent_history = vec![12.0, 11.5, 12.5, 11.8, 12.2];
        for &points in &consistent_history {
            let _price = calculator.calculate_fair2_with_consistency(
                &mut consistent_perf, points, &consistent_history
            ).unwrap();
        }
        
        // Volatile performance (high volatility)
        let volatile_history = vec![5.0, 25.0, 3.0, 30.0, 2.0];
        for &points in &volatile_history {
            let _price = calculator.calculate_fair2_with_consistency(
                &mut volatile_perf, points, &volatile_history
            ).unwrap();
        }
        
        // Both should have valid prices
        assert!(consistent_perf.fair_cents > 0);
        assert!(volatile_perf.fair_cents > 0);
    }

    #[test]
    fn test_rolling_stddev() {
        let config = create_test_config();
        let calculator = PriceCalculator::new(config);
        
        // Test consistent data
        let consistent = vec![10.0, 10.5, 9.5, 10.2, 9.8];
        let stddev_consistent = calculator.calculate_rolling_stddev(&consistent);
        assert!(stddev_consistent < 1.0); // Low volatility
        
        // Test volatile data
        let volatile = vec![5.0, 20.0, 3.0, 25.0, 2.0];
        let stddev_volatile = calculator.calculate_rolling_stddev(&volatile);
        assert!(stddev_volatile > 5.0); // High volatility
        
        // Test edge cases
        assert_eq!(calculator.calculate_rolling_stddev(&[]), 0.0);
        assert_eq!(calculator.calculate_rolling_stddev(&[10.0]), 0.0);
    }
}

/// Leaderboard-based price calculator - primary price driver
/// Maps season performance rankings to target prices ($50-$150 range)
pub struct LeaderboardCalculator {
    config: RpeConfig,
}

/// Player performance data for leaderboard calculations
#[derive(Debug, Clone)]
pub struct PlayerPerformance {
    pub player_id: i32,
    pub name: String,
    pub position: String,
    pub total_points: f64,
    pub games_played: u32,
    pub ppg_raw: f64,
    pub ppg_shrunk: f64,
    pub recent_form: f64, // Last 3 weeks average
    pub projection: f64,
    pub nfl_rank: u32, // NFL Fantasy leaderboard rank (1 = best)
    pub weekly_ranks: Vec<(u32, u32)>, // (week, rank) pairs
}

/// Leaderboard cohort statistics
#[derive(Debug)]
pub struct CohortStats {
    pub total_points: Vec<f64>,
    pub ppg_shrunk: Vec<f64>,
    pub recent_form: Vec<f64>,
    pub projections: Vec<f64>,
}

impl LeaderboardCalculator {
    /// Create a new leaderboard calculator
    pub fn new(config: RpeConfig) -> Self {
        Self { config }
    }

    /// Calculate NFL leaderboard-based target price for a player
    /// Uses real NFL Fantasy rankings as primary driver with weekly momentum
    pub fn calculate_leaderboard_price(
        &self,
        player_perf: &PlayerPerformance,
        cohort_stats: &CohortStats,
        week: u32,
    ) -> Result<f64> {
        // Base price from NFL Fantasy leaderboard ranking with better differentiation
        let nfl_rank_score = self.calculate_rank_score(player_perf.nfl_rank);
        let base_price = self.map_to_target_price(nfl_rank_score);
        
        // Weekly momentum adjustment (smaller impact)
        let momentum_adjustment = self.calculate_weekly_momentum(player_perf, week);
        
        // Final price = Base price + momentum adjustment
        let final_price = base_price + momentum_adjustment;
        
        // Keep season projections as a small background factor
        let proj_adjustment = self.calculate_projection_adjustment(player_perf, cohort_stats, week);
        
        let final_price_with_proj = final_price + proj_adjustment;

        info!(
            "NFL Leaderboard price for {}: ${:.2} (NFL Rank: {}, Score: {:.3}, Base: ${:.2}, Momentum: ${:.2}, Proj: ${:.2})",
            player_perf.name, final_price_with_proj, player_perf.nfl_rank, nfl_rank_score, base_price, momentum_adjustment, proj_adjustment
        );

        Ok(final_price_with_proj)
    }

    /// Map leaderboard score to target price using steep gamma curve for better differentiation
    fn map_to_target_price(&self, leaderboard_score: f64) -> f64 {
        let p_min = 25.0;  // Much lower minimum for deep bench players
        let p_max = 200.0; // Much higher maximum for elite players
        let gamma = 2.5;   // Much steeper curve for better differentiation

        p_min + (p_max - p_min) * (1.0 - leaderboard_score.powf(gamma))
    }

    /// Calculate weekly momentum adjustment based on recent weekly rankings
    fn calculate_weekly_momentum(&self, player_perf: &PlayerPerformance, _week: u32) -> f64 {
        if player_perf.weekly_ranks.is_empty() {
            return 0.0;
        }

        // Get recent weekly ranks (last 3 weeks)
        let recent_ranks: Vec<u32> = player_perf.weekly_ranks.iter()
            .rev()
            .take(3)
            .map(|(_, rank)| *rank)
            .collect();

        if recent_ranks.is_empty() {
            return 0.0;
        }

        // Calculate momentum: better recent ranks = positive adjustment
        let avg_recent_rank = recent_ranks.iter().sum::<u32>() as f64 / recent_ranks.len() as f64;
        let momentum = (250.0 - avg_recent_rank) / 250.0; // Normalize to -1 to 1 range
        
        // Scale momentum to dollar adjustment (±$10 max)
        momentum * 10.0
    }

    /// Calculate rank score with better differentiation between tiers
    fn calculate_rank_score(&self, rank: u32) -> f64 {
        // Create much better differentiation between tiers
        // Top 10 players: 0.0 - 0.1 (elite tier)
        // Top 50 players: 0.1 - 0.3 (high tier) 
        // Top 100 players: 0.3 - 0.5 (mid-high tier)
        // Top 200 players: 0.5 - 0.7 (mid tier)
        // Rest: 0.7 - 1.0 (low tier)
        
        if rank <= 10 {
            // Elite tier: very low scores
            (rank as f64 - 1.0) / 100.0 // 0.0 to 0.09
        } else if rank <= 50 {
            // High tier: low scores
            0.1 + ((rank - 10) as f64 / 40.0) * 0.2 // 0.1 to 0.3
        } else if rank <= 100 {
            // Mid-high tier: medium-low scores
            0.3 + ((rank - 50) as f64 / 50.0) * 0.2 // 0.3 to 0.5
        } else if rank <= 200 {
            // Mid tier: medium scores
            0.5 + ((rank - 100) as f64 / 100.0) * 0.2 // 0.5 to 0.7
        } else {
            // Low tier: high scores
            0.7 + ((rank - 200) as f64 / 300.0) * 0.3 // 0.7 to 1.0
        }
    }

    /// Calculate small projection adjustment to keep season projections relevant
    fn calculate_projection_adjustment(&self, player_perf: &PlayerPerformance, cohort_stats: &CohortStats, week: u32) -> f64 {
        // Fast fade of projections as season progresses
        let alpha_proj = (-0.25 * week as f64).exp(); // λ = 0.25
        let proj_weight = 0.10 * alpha_proj; // Max 10% weight, fading to ~1% by week 6
        
        // Calculate projection percentile
        let q_proj = self.percentile_desc(player_perf.projection, &cohort_stats.projections);
        
        // Small adjustment based on projection vs actual performance
        let proj_adjustment = (0.5 - q_proj) * proj_weight * 20.0; // ±$2 max adjustment
        
        proj_adjustment
    }

    /// Calculate percentile in descending order (0 = best, 1 = worst)
    fn percentile_desc(&self, value: f64, cohort: &[f64]) -> f64 {
        if cohort.is_empty() {
            return 0.5; // Default to median if no data
        }

        let mut sorted = cohort.to_vec();
        sorted.sort_by(|a, b| b.partial_cmp(a).unwrap()); // Descending

        let rank = sorted.iter().position(|&x| x <= value).unwrap_or(sorted.len());
        rank as f64 / (sorted.len() - 1) as f64
    }

    /// Build player performance data from weekly stats with NFL rankings
    pub fn build_player_performance_with_rankings(
        &self,
        player_id: i32,
        name: &str,
        position: &str,
        projection: f64,
        weekly_points: &[(u32, f64)], // (week, points)
        nfl_rank: u32,
        weekly_ranks: &[(u32, u32)], // (week, rank)
    ) -> PlayerPerformance {
        let total_points: f64 = weekly_points.iter().map(|(_, pts)| pts).sum();
        let games_played = weekly_points.len() as u32;
        let ppg_raw = if games_played > 0 { total_points / games_played as f64 } else { 0.0 };
        
        // PPG with shrinkage (G0 = 3)
        let g0 = 3.0;
        let ppg_shrunk = if games_played > 0 {
            (games_played as f64 / (games_played as f64 + g0)) * ppg_raw
                + (g0 / (games_played as f64 + g0)) * (projection / 17.0)
        } else {
            projection / 17.0
        };

        // Recent form (last 3 weeks)
        let recent_weeks: Vec<f64> = weekly_points.iter()
            .rev()
            .take(3)
            .map(|(_, pts)| *pts)
            .collect();
        let recent_form = if !recent_weeks.is_empty() {
            recent_weeks.iter().sum::<f64>() / recent_weeks.len() as f64
        } else {
            0.0
        };

        PlayerPerformance {
            player_id,
            name: name.to_string(),
            position: position.to_string(),
            total_points,
            games_played,
            ppg_raw,
            ppg_shrunk,
            recent_form,
            projection,
            nfl_rank,
            weekly_ranks: weekly_ranks.to_vec(),
        }
    }

    /// Build player performance data from weekly stats (legacy method)
    pub fn build_player_performance(
        &self,
        player_id: i32,
        name: &str,
        position: &str,
        projection: f64,
        weekly_points: &[(u32, f64)], // (week, points)
    ) -> PlayerPerformance {
        let total_points: f64 = weekly_points.iter().map(|(_, pts)| pts).sum();
        let games_played = weekly_points.len() as u32;
        let ppg_raw = if games_played > 0 { total_points / games_played as f64 } else { 0.0 };
        
        // PPG with shrinkage (G0 = 3)
        let g0 = 3.0;
        let ppg_shrunk = if games_played > 0 {
            (games_played as f64 / (games_played as f64 + g0)) * ppg_raw
                + (g0 / (games_played as f64 + g0)) * (projection / 17.0)
        } else {
            projection / 17.0
        };

        // Recent form (last 3 weeks)
        let recent_weeks: Vec<f64> = weekly_points.iter()
            .rev()
            .take(3)
            .map(|(_, pts)| *pts)
            .collect();
        let recent_form = if !recent_weeks.is_empty() {
            recent_weeks.iter().sum::<f64>() / recent_weeks.len() as f64
        } else {
            0.0
        };

        PlayerPerformance {
            player_id,
            name: name.to_string(),
            position: position.to_string(),
            total_points,
            games_played,
            ppg_raw,
            ppg_shrunk,
            recent_form,
            projection,
            nfl_rank: 500, // Default rank for legacy method
            weekly_ranks: Vec::new(), // Empty for legacy method
        }
    }

    /// Build cohort statistics for all players
    pub fn build_cohort_stats(&self, players: &[PlayerPerformance]) -> CohortStats {
        CohortStats {
            total_points: players.iter().map(|p| p.total_points).collect(),
            ppg_shrunk: players.iter().map(|p| p.ppg_shrunk).collect(),
            recent_form: players.iter().map(|p| p.recent_form).collect(),
            projections: players.iter().map(|p| p.projection).collect(),
        }
    }
}
