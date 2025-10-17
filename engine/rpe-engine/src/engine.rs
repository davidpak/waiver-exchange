use anyhow::Context;
use std::collections::HashMap;
use std::fs;
use sqlx::PgPool;
use tracing::info;
use bigdecimal::{ToPrimitive, FromPrimitive};

use crate::{
    config::RpeConfig,
    calculator::{PriceCalculator, LeaderboardCalculator, PlayerPerformance},
    models::{SeasonProjectionJson, WeeklyPlayerData, RpeEvent},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct SeasonProjectionsJson {
    season: String,
    last_updated: String,
    players: Vec<SeasonProjectionJson>,
}

pub struct RpeEngine {
    config: RpeConfig,
    calculator: PriceCalculator,
    leaderboard_calc: LeaderboardCalculator,
    pool: PgPool,
    last_prices: HashMap<i32, i64>,
}

impl RpeEngine {
    pub async fn new(config: RpeConfig) -> anyhow::Result<Self> {
        info!("🔧 Creating RPE Engine with Leaderboard-First Pricing");
        
        let calculator = PriceCalculator::new(config.clone());
        let leaderboard_calc = LeaderboardCalculator::new(config.clone());
        let pool = PgPool::connect(&config.database.url)
            .await
            .context("Failed to connect to database")?;
        
        info!("✅ RPE Engine created successfully");
        
        Ok(Self {
            config,
            calculator,
            leaderboard_calc,
            pool,
            last_prices: HashMap::new(),
        })
    }

    /// Process Fair Price 2.3 - NFL Leaderboard + Weekly Rankings + Season Projections
    /// Combines real NFL rankings with weekly performance and season projections
    pub async fn process_fair_price_2_0(&mut self) -> anyhow::Result<Vec<RpeEvent>> {
        info!("🚀 Starting Fair Price 2.3 - NFL Leaderboard + Weekly Rankings + Season Projections");
        
        // Load season projections from JSON (relative to project root)
        let projections_content = fs::read_to_string("../../data/players/season_projections_2025.json")?;
        let projections_data: SeasonProjectionsJson = serde_json::from_str(&projections_content)?;
        
        // Load NFL Fantasy leaderboard data
        let leaderboard_content = fs::read_to_string("../../data/players/nfl_fantasy_leaderboard_2025.json")?;
        let leaderboard_data: serde_json::Value = serde_json::from_str(&leaderboard_content)?;
        let nfl_players: Vec<serde_json::Value> = leaderboard_data["players"].as_array().unwrap().clone();
        
        // Load weekly data from JSON files
        let mut weekly_data_map = HashMap::new();
        for week in 1..=6 {
            let filename = format!("../../data/players/week_{}_stats_2025.json", week);
            if std::path::Path::new(&filename).exists() {
                let weekly_content = fs::read_to_string(&filename)?;
                let weekly_data: WeeklyPlayerData = serde_json::from_str(&weekly_content)?;
                weekly_data_map.insert(week, weekly_data);
            }
        }

        info!("📊 Loaded {} season projections", projections_data.players.len());
        info!("🏆 Loaded {} NFL leaderboard players", nfl_players.len());
        info!("📅 Loaded weekly data for {} weeks", weekly_data_map.len());

        // Build player performance data for all players
        let mut player_performances = Vec::new();
        for projection in &projections_data.players {
            // Find NFL leaderboard ranking for this player
            let nfl_rank = nfl_players.iter()
                .find(|p| p["name"].as_str() == Some(&projection.name))
                .and_then(|p| p["rank"].as_u64())
                .unwrap_or(500) as u32; // Default to rank 500 if not found
            
            // Find weekly data for this player by symbol_id
            let mut player_weekly_points = Vec::new();
            let mut player_weekly_ranks = Vec::new();
            for (week, weekly_data) in &weekly_data_map {
                if let Some(player) = weekly_data.players.iter().find(|p| p.symbol_id == Some(projection.symbol_id)) {
                    player_weekly_points.push((*week, player.fantasy_points));
                    if let Some(rank) = player.rank {
                        player_weekly_ranks.push((*week, rank));
                    }
                }
            }

            let player_perf = self.leaderboard_calc.build_player_performance_with_rankings(
                projection.symbol_id as i32,
                &projection.name,
                &projection.position,
                projection.projected_points.to_f64().unwrap_or(0.0),
                &player_weekly_points,
                nfl_rank,
                &player_weekly_ranks,
            );
            player_performances.push(player_perf);
        }

        // Build cohort statistics
        let cohort_stats = self.leaderboard_calc.build_cohort_stats(&player_performances);
        info!("📈 Built cohort stats for {} players", player_performances.len());

        let mut events = Vec::new();
        let mut processed_count = 0;
        let mut updated_count = 0;

        info!("🔄 Processing {} players with leaderboard-first pricing...", player_performances.len());
        
        for player_perf in &player_performances {
            info!("Processing player: {} (ID: {}, Total: {:.1} pts, PPG: {:.1})", 
                  player_perf.name, player_perf.player_id, player_perf.total_points, player_perf.ppg_shrunk);

            // Calculate leaderboard-based target price (primary driver - 80% weight)
            let leaderboard_price = self.leaderboard_calc.calculate_leaderboard_price(
                player_perf,
                &cohort_stats,
                6, // Current week
            )?;

            // Calculate momentum adjustment (background - 20% weight)
            let momentum_adjustment = self.calculate_momentum_adjustment(player_perf)?;

            // Final price = Leaderboard price + small momentum adjustment
            let final_price_dollars = leaderboard_price + momentum_adjustment;
            let final_price_cents = (final_price_dollars * 100.0) as i64;

            info!("💰 Final price for {}: ${:.2} (Leaderboard: ${:.2}, Momentum: ${:.2})", 
                  player_perf.name, final_price_dollars, leaderboard_price, momentum_adjustment);

            // Store in database
            let result = sqlx::query!(
                r#"
                INSERT INTO rpe_fair_prices (player_id, ts, season, week, fair_cents, band_bps, kappa_cents_per_pt, pacing_mode, actual_pts, delta_pts, reason)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT (player_id) DO UPDATE SET
                    ts = EXCLUDED.ts,
                    season = EXCLUDED.season,
                    week = EXCLUDED.week,
                    fair_cents = EXCLUDED.fair_cents,
                    band_bps = EXCLUDED.band_bps,
                    kappa_cents_per_pt = EXCLUDED.kappa_cents_per_pt,
                    pacing_mode = EXCLUDED.pacing_mode,
                    actual_pts = EXCLUDED.actual_pts,
                    delta_pts = EXCLUDED.delta_pts,
                    reason = EXCLUDED.reason
                "#,
                player_perf.player_id,
                chrono::Utc::now(),
                self.config.rpe.season,
                None::<i32>, // No specific week for Fair Price 2.2
                final_price_cents,
                self.config.rpe.ingame_band_bps as i32,
                0, // Not using kappa in leaderboard pricing
                "fair_price_2_2_leaderboard",
                bigdecimal::BigDecimal::from_f64(player_perf.total_points).unwrap_or_default(),
                bigdecimal::BigDecimal::from_f64(momentum_adjustment).unwrap_or_default(),
                serde_json::json!({
                    "fair_price_2_2": {
                        "leaderboard_price": leaderboard_price,
                        "momentum_adjustment": momentum_adjustment,
                        "total_points": player_perf.total_points,
                        "ppg_shrunk": player_perf.ppg_shrunk,
                        "recent_form": player_perf.recent_form,
                        "games_played": player_perf.games_played
                    }
                }),
            )
            .execute(&self.pool)
            .await
            .context("Failed to store fair price")?;
            
            info!("✅ Successfully updated database: {} rows affected", result.rows_affected());
            
            // Check if price changed significantly
            let last_price = self.last_prices.get(&player_perf.player_id).copied().unwrap_or(0);
            let delta_cents = final_price_cents - last_price;
            
            if delta_cents.abs() >= self.config.events.min_change_cents as i64 {
                self.last_prices.insert(player_perf.player_id, final_price_cents);
                updated_count += 1;
                
                let reason = crate::models::CalculationReason::LeaderboardBased {
                    leaderboard_score: 0.0, // Will be calculated properly later
                    total_points: player_perf.total_points,
                    ppg_shrunk: player_perf.ppg_shrunk,
                    recent_form: player_perf.recent_form,
                };
                
                events.push(RpeEvent::FairPriceUpdated {
                    player_id: player_perf.player_id,
                    fair_cents: final_price_cents,
                    delta_cents,
                    reason,
                    timestamp: chrono::Utc::now(),
                });
            }
            
            processed_count += 1;
        }
        
        events.push(RpeEvent::BatchCompleted {
            processed_count,
            updated_count,
            timestamp: chrono::Utc::now(),
        });
        
        info!("✅ Fair Price 2.2 processing complete: {} processed, {} updated", processed_count, updated_count);
        Ok(events)
    }

    /// Calculate small momentum adjustment (background factor)
    fn calculate_momentum_adjustment(&self, player_perf: &PlayerPerformance) -> anyhow::Result<f64> {
        // Simple momentum based on recent form vs season average
        let season_avg = if player_perf.games_played > 0 {
            player_perf.total_points / player_perf.games_played as f64
        } else {
            0.0
        };

        let momentum = player_perf.recent_form - season_avg;
        
        // Cap momentum adjustment at ±$10
        let adjustment = (momentum * 0.5).clamp(-10.0, 10.0);
        
        Ok(adjustment)
    }
}