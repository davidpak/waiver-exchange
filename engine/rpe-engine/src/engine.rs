use crate::config::RpeConfig;
use crate::models::*;
use crate::calculator::PriceCalculator;
use anyhow::{Context, Result};
use bigdecimal::{BigDecimal, FromPrimitive};
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{info, warn, error};

/// Main RPE Engine
pub struct RpeEngine {
    config: RpeConfig,
    calculator: PriceCalculator,
    pool: PgPool,
    last_prices: HashMap<i32, i64>, // player_id -> last_fair_cents
}

impl RpeEngine {
    /// Create a new RPE engine
    pub async fn new(config: RpeConfig) -> Result<Self> {
        let pool = PgPool::connect(&config.database.url)
            .await
            .context("Failed to connect to database")?;
        
        let calculator = PriceCalculator::new(config.clone());
        
        Ok(Self {
            config,
            calculator,
            pool,
            last_prices: HashMap::new(),
        })
    }
    
    /// Process season projections and calculate initial F₀ prices
    pub async fn process_season_projections(&mut self) -> Result<Vec<RpeEvent>> {
        info!("Processing season projections for initial F₀ calculations");
        
        let projections = self.load_season_projections().await?;
        let mut events = Vec::new();
        let mut processed_count = 0;
        let mut updated_count = 0;
        
        for projection in projections {
            match self.calculator.calculate_from_projection(&projection) {
                Ok(calculation) => {
                    // Store the fair price (only if no recent live data exists)
                    if let Err(e) = self.store_fair_price_if_no_live_data(&calculation).await {
                        error!("Failed to store fair price for player {}: {}", calculation.player_id, e);
                        events.push(RpeEvent::CalculationFailed {
                            player_id: calculation.player_id,
                            error: e.to_string(),
                            timestamp: chrono::Utc::now(),
                        });
                        continue;
                    }
                    
                    // Check if price changed significantly
                    let last_price = self.last_prices.get(&calculation.player_id).copied().unwrap_or(0);
                    let delta_cents = calculation.ft_cents - last_price;
                    
                    if delta_cents.abs() >= self.config.events.min_change_cents {
                        self.last_prices.insert(calculation.player_id, calculation.ft_cents);
                        updated_count += 1;
                        
                        events.push(RpeEvent::FairPriceUpdated {
                            player_id: calculation.player_id,
                            fair_cents: calculation.ft_cents,
                            delta_cents,
                            reason: calculation.reason,
                            timestamp: chrono::Utc::now(),
                        });
                    }
                    
                    processed_count += 1;
                }
                Err(e) => {
                    error!("Failed to calculate price for player {}: {}", projection.player_id, e);
                    events.push(RpeEvent::CalculationFailed {
                        player_id: projection.player_id,
                        error: e.to_string(),
                        timestamp: chrono::Utc::now(),
                    });
                }
            }
        }
        
        events.push(RpeEvent::BatchCompleted {
            processed_count,
            updated_count,
            timestamp: chrono::Utc::now(),
        });
        
        info!("Processed {} projections, updated {} prices", processed_count, updated_count);
        Ok(events)
    }
    
    /// Process player week points and update Fₜ prices
    pub async fn process_player_week_points(&mut self, week: u32) -> Result<Vec<RpeEvent>> {
        info!("Processing player week points for week {}", week);
        
        let week_points = self.load_player_week_points(week).await?;
        let mut events = Vec::new();
        let mut processed_count = 0;
        let mut updated_count = 0;
        
        // Group by player_id and get latest points
        let mut latest_points: HashMap<i32, PlayerWeekPoints> = HashMap::new();
        for points in week_points {
            let entry = latest_points.entry(points.player_id).or_insert_with(|| points.clone());
            if points.ts > entry.ts {
                *entry = points;
            }
        }
        
        for (player_id, current_points) in latest_points {
            // Get previous points for delta calculation
            let prev_points = self.get_previous_week_points(player_id, week).await?;
            
            // Get the player's position and F₀
            let (position, f0_cents) = match self.get_player_info(player_id).await? {
                Some((pos, f0)) => (pos, f0),
                None => {
                    warn!("No player info found for player {}", player_id);
                    continue;
                }
            };
            
            let calculation = if let Some(prev) = prev_points {
                // Calculate with delta from previous week
                self.calculator.calculate_from_delta(
                    player_id,
                    &position,
                    f0_cents,
                    current_points.fantasy_pts.clone(),
                    prev.fantasy_pts,
                )?
            } else {
                // No previous points, but we still have F₀ from season projection
                // Just use the current points as the baseline for future deltas
                self.calculator.calculate_from_delta(
                    player_id,
                    &position,
                    f0_cents,
                    current_points.fantasy_pts.clone(),
                    current_points.fantasy_pts.clone(), // No delta on first week
                )?
            };
            
            // Store the fair price
            if let Err(e) = self.store_fair_price(&calculation).await {
                error!("Failed to store fair price for player {}: {}", player_id, e);
                events.push(RpeEvent::CalculationFailed {
                    player_id,
                    error: e.to_string(),
                    timestamp: chrono::Utc::now(),
                });
                continue;
            }
            
            // Check if price changed significantly
            let last_price = self.last_prices.get(&player_id).copied().unwrap_or(0);
            let delta_cents = calculation.ft_cents - last_price;
            
            if delta_cents.abs() >= self.config.events.min_change_cents {
                self.last_prices.insert(player_id, calculation.ft_cents);
                updated_count += 1;
                
                events.push(RpeEvent::FairPriceUpdated {
                    player_id,
                    fair_cents: calculation.ft_cents,
                    delta_cents,
                    reason: calculation.reason,
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
        
        info!("Processed {} players, updated {} prices", processed_count, updated_count);
        Ok(events)
    }
    
    /// Load season projections from database
    async fn load_season_projections(&self) -> Result<Vec<SeasonProjection>> {
        let projections = sqlx::query_as!(
            SeasonProjection,
            r#"
            SELECT player_id, season, proj_points, fantasy_pos, adp, source, ingested_at
            FROM projections_season
            WHERE season = $1
            ORDER BY player_id
            "#,
            self.config.rpe.season
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to load season projections")?;
        
        info!("Loaded {} season projections", projections.len());
        Ok(projections)
    }
    
    /// Load player week points from database
    async fn load_player_week_points(&self, week: u32) -> Result<Vec<PlayerWeekPoints>> {
        let points = sqlx::query_as!(
            PlayerWeekPoints,
            r#"
            SELECT player_id, season, week, ts, fantasy_pts, is_game_over, raw
            FROM player_week_points
            WHERE season = $1 AND week = $2
            ORDER BY player_id, ts
            "#,
            self.config.rpe.season,
            week as i32
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to load player week points")?;
        
        info!("Loaded {} player week points for week {}", points.len(), week);
        Ok(points)
    }
    
    /// Get previous week points for a player
    async fn get_previous_week_points(&self, player_id: i32, current_week: u32) -> Result<Option<PlayerWeekPoints>> {
        let prev_week = current_week - 1;
        if prev_week == 0 {
            return Ok(None);
        }
        
        let points = sqlx::query_as!(
            PlayerWeekPoints,
            r#"
            SELECT player_id, season, week, ts, fantasy_pts, is_game_over, raw
            FROM player_week_points
            WHERE player_id = $1 AND season = $2 AND week = $3
            ORDER BY ts DESC
            LIMIT 1
            "#,
            player_id,
            self.config.rpe.season,
            prev_week as i32
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to load previous week points")?;
        
        Ok(points)
    }
    
    /// Get player info (position and F₀) from database
    async fn get_player_info(&self, player_id: i32) -> Result<Option<(String, i64)>> {
        // Try to get from projections first
        let projection = sqlx::query!(
            r#"
            SELECT fantasy_pos, proj_points
            FROM projections_season
            WHERE player_id = $1 AND season = $2
            "#,
            player_id,
            self.config.rpe.season
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to load player projection")?;
        
        if let Some(proj) = projection {
            let f0_cents = self.calculator.calculate_f0(&SeasonProjection {
                player_id,
                season: self.config.rpe.season,
                proj_points: proj.proj_points,
                fantasy_pos: proj.fantasy_pos.clone(),
                adp: None,
                source: "lookup".to_string(),
                ingested_at: chrono::Utc::now(),
            })?;
            
            return Ok(Some((proj.fantasy_pos, f0_cents)));
        }
        
        // Fallback: try to get from player_id_mapping
        let mapping = sqlx::query!(
            r#"
            SELECT position
            FROM player_id_mapping
            WHERE sportsdataio_player_id = $1
            "#,
            player_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to load player mapping")?;
        
        if let Some(map) = mapping {
            // Use default F₀ for unmapped players
            let f0_cents = self.config.rpe.base_cents;
            return Ok(Some((map.position.unwrap_or_default(), f0_cents)));
        }
        
        Ok(None)
    }
    
    /// Store fair price in database (only if no recent live data exists)
    async fn store_fair_price_if_no_live_data(&self, calculation: &RpeCalculation) -> Result<()> {
        // Check if we have recent live data (within last 10 minutes)
        let live_threshold = chrono::Utc::now() - chrono::Duration::minutes(10);
        
        let has_recent_live_data = sqlx::query!(
            r#"
            SELECT player_id
            FROM rpe_fair_prices
            WHERE player_id = $1 
            AND ts > $2
            AND reason->>'source' = 'live_points'
            LIMIT 1
            "#,
            calculation.player_id,
            live_threshold
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to check for recent live data")?;
        
        // If we have recent live data, skip storing projection-based price
        if has_recent_live_data.is_some() {
            return Ok(());
        }
        
        // Store the projection-based price
        self.store_fair_price(calculation).await
    }
    
    /// Store fair price in database (upsert - update existing or insert new)
    async fn store_fair_price(&self, calculation: &RpeCalculation) -> Result<()> {
        let record = FairPriceRecord::new(
            calculation.player_id,
            self.config.rpe.season,
            Some(self.config.rpe.week as i32),
            calculation.ft_cents,
            self.config.rpe.ingame_band_bps,
            calculation.kappa,
            self.config.rpe.pacing_mode.clone(),
            calculation.actual_pts.clone(),
            calculation.delta_pts.clone(),
            calculation.reason.clone(),
        );
        
        // Determine source based on reason
        let source = match &calculation.reason {
            crate::models::CalculationReason::Projection { .. } => "projection",
            crate::models::CalculationReason::FantasyPointsDelta { .. } => "live_points",
            crate::models::CalculationReason::PacingStep { .. } => "hybrid",
        };
        
        info!("Storing fair price for player {}: {} cents, source: {}", 
              calculation.player_id, calculation.ft_cents, source);
        
        // Upsert: update existing record or insert new one
        let result = sqlx::query!(
            r#"
            INSERT INTO rpe_fair_prices (player_id, ts, season, week, fair_cents, band_bps, kappa_cents_per_pt, pacing_mode, actual_pts, delta_pts, reason, source, confidence_score)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (player_id) 
            DO UPDATE SET
                ts = EXCLUDED.ts,
                season = EXCLUDED.season,
                week = EXCLUDED.week,
                fair_cents = EXCLUDED.fair_cents,
                band_bps = EXCLUDED.band_bps,
                kappa_cents_per_pt = EXCLUDED.kappa_cents_per_pt,
                pacing_mode = EXCLUDED.pacing_mode,
                actual_pts = EXCLUDED.actual_pts,
                delta_pts = EXCLUDED.delta_pts,
                reason = EXCLUDED.reason,
                source = EXCLUDED.source,
                confidence_score = EXCLUDED.confidence_score
            "#,
            record.player_id,
            record.ts,
            record.season,
            record.week,
            record.fair_cents,
            record.band_bps as i32,
            record.kappa_cents_per_pt as i32,
            record.pacing_mode,
            record.actual_pts,
            record.delta_pts,
            record.reason,
            source,
            BigDecimal::from_f64(0.8).unwrap_or_default() // Default confidence score
        )
        .execute(&self.pool)
        .await
        .context("Failed to store fair price")?;
        
        info!("Successfully stored fair price for player {}: {} rows affected", 
              calculation.player_id, result.rows_affected());
        
        Ok(())
    }
}
