use crate::config::FetcherConfig;
use crate::models::*;
use anyhow::{Context, Result};
use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive};
use chrono::{Duration, Utc};
use reqwest::Client;
use sqlx::PgPool;
use std::time::Duration as StdDuration;
use tracing::{info, error};

/// Main SportsDataIO Fetcher service
pub struct SportsDataIOFetcher {
    config: FetcherConfig,
    client: Client,
    pool: PgPool,
}

impl SportsDataIOFetcher {
    /// Create a new fetcher instance
    pub async fn new(config: FetcherConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(StdDuration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;
        
        let pool = PgPool::connect(&config.database.url)
            .await
            .context("Failed to connect to database")?;
        
        Ok(Self {
            config,
            client,
            pool,
        })
    }
    
    /// Fetch season projections from SportsDataIO
    pub async fn fetch_season_projections(&self) -> Result<Vec<SeasonProjection>> {
        // Use embedded API key from documentation
        let api_key = "2d60a5317f014813810755b281f8c2ea";
        // Remove "REG" suffix from season (e.g., "2025REG" -> "2025")
        let season = self.config.sportsdataio.season.replace("REG", "");
        let url = format!(
            "https://api.sportsdata.io/v3/nfl/projections/json/PlayerSeasonProjectionStats/{}?key={}",
            season,
            api_key
        );
        
        info!("Fetching season projections from: {}", url);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch season projections")?;
        
        if !response.status().is_success() {
            anyhow::bail!("API request failed with status: {}", response.status());
        }
        
        let projections: Vec<SeasonProjection> = response
            .json()
            .await
            .context("Failed to parse season projections JSON")?;
        
        info!("Successfully fetched {} season projections", projections.len());
        Ok(projections)
    }
    
    /// Fetch player game stats for a specific week
    pub async fn fetch_player_game_stats(&self, week: u32) -> Result<Vec<PlayerGameStats>> {
        // Use embedded API key from documentation
        let api_key = "6a0d677700b24336990b4525be87ca82";
        // Remove "REG" suffix from season (e.g., "2025REG" -> "2025")
        let season = self.config.sportsdataio.season.replace("REG", "");
        let url = format!(
            "https://api.sportsdata.io/v3/nfl/stats/json/PlayerGameStatsByWeek/{}/{}?key={}",
            season,
            week,
            api_key
        );
        
        info!("Fetching player game stats for week {} from: {}", week, url);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch player game stats")?;
        
        if !response.status().is_success() {
            anyhow::bail!("API request failed with status: {}", response.status());
        }
        
        let stats: Vec<PlayerGameStats> = response
            .json()
            .await
            .context("Failed to parse player game stats JSON")?;
        
        info!("Successfully fetched {} player game stats for week {}", stats.len(), week);
        Ok(stats)
    }
    
    /// Store season projections in database (only for players in our mapping table)
    pub async fn store_season_projections(&self, projections: &[SeasonProjection]) -> Result<usize> {
        let season = self.config.sportsdataio.season
            .replace("REG", "")
            .parse::<i32>()
            .context("Invalid season format")?;
        
        // Get list of player IDs that exist in our mapping table
        let mapped_player_ids: Vec<i32> = sqlx::query_scalar!(
            "SELECT sportsdataio_player_id FROM player_id_mapping"
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to load mapped player IDs")?;
        
        let mapped_set: std::collections::HashSet<i32> = mapped_player_ids.into_iter().collect();
        info!("Found {} mapped players in our system", mapped_set.len());
        
        let mut stored_count = 0;
        let mut filtered_count = 0;
        
        for projection in projections {
            // Only process players that exist in our mapping table
            if !mapped_set.contains(&projection.player_id) {
                filtered_count += 1;
                continue;
            }
            
            // Convert to database record
            let record = projection.to_projection_record(season);
            
            // Upsert into projections_season table
            sqlx::query!(
                r#"
                INSERT INTO projections_season (player_id, season, proj_points, fantasy_pos, adp, source, ingested_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (player_id, season) 
                DO UPDATE SET
                    proj_points = EXCLUDED.proj_points,
                    fantasy_pos = EXCLUDED.fantasy_pos,
                    adp = EXCLUDED.adp,
                    source = EXCLUDED.source,
                    ingested_at = EXCLUDED.ingested_at
                "#,
                record.player_id,
                record.season,
                BigDecimal::from_f64(record.proj_points).unwrap_or_default(),
                record.fantasy_pos,
                record.adp.and_then(|v| BigDecimal::from_f64(v)),
                record.source,
                record.ingested_at
            )
            .execute(&self.pool)
            .await
            .with_context(|| format!("Failed to store projection for player {}", record.player_id))?;
            
            stored_count += 1;
        }
        
        info!("Stored {} season projections in database (filtered out {} unmapped players)", stored_count, filtered_count);
        Ok(stored_count)
    }
    
    /// Store player week points in database (only for players in our mapping table)
    pub async fn store_player_week_points(&self, stats: &[PlayerGameStats], week: u32) -> Result<usize> {
        let season = self.config.sportsdataio.season
            .replace("REG", "")
            .parse::<i32>()
            .context("Invalid season format")?;
        
        // Get list of player IDs that exist in our mapping table
        let mapped_player_ids: Vec<i32> = sqlx::query_scalar!(
            "SELECT sportsdataio_player_id FROM player_id_mapping"
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to load mapped player IDs")?;
        
        let mapped_set: std::collections::HashSet<i32> = mapped_player_ids.into_iter().collect();
        info!("Found {} mapped players in our system", mapped_set.len());
        
        let mut stored_count = 0;
        let mut filtered_count = 0;
        
        for stat in stats {
            // Only process players that exist in our mapping table
            if !mapped_set.contains(&stat.player_id) {
                filtered_count += 1;
                continue;
            }
            
            // Convert to database record
            let record = stat.to_week_points_record(season, week as i32);
            
            // Check if we already have a recent record for this player/week (within last 5 minutes)
            let recent_threshold = chrono::Utc::now() - chrono::Duration::minutes(5);
            
            let existing_record = sqlx::query!(
                r#"
                SELECT fantasy_pts, is_game_over
                FROM player_week_points
                WHERE player_id = $1 AND season = $2 AND week = $3 AND ts > $4
                ORDER BY ts DESC
                LIMIT 1
                "#,
                record.player_id,
                record.season,
                record.week,
                recent_threshold
            )
            .fetch_optional(&self.pool)
            .await
            .with_context(|| format!("Failed to check existing record for player {}", record.player_id))?;
            
            // Only insert if we don't have a recent record, or if the fantasy points have changed
            let should_insert = match existing_record {
                None => true, // No recent record, insert
                Some(existing) => {
                    // Check if fantasy points or game status changed
                    let existing_pts = existing.fantasy_pts.to_f64().unwrap_or(0.0);
                    let new_pts = record.fantasy_pts;
                    let pts_changed = (existing_pts - new_pts).abs() > 0.01; // Allow small floating point differences
                    let game_status_changed = existing.is_game_over != record.is_game_over;
                    
                    pts_changed || game_status_changed
                }
            };
            
            if should_insert {
                sqlx::query!(
                    r#"
                    INSERT INTO player_week_points (player_id, season, week, ts, fantasy_pts, is_game_over, raw)
                    VALUES ($1, $2, $3, $4, $5, $6, $7)
                    "#,
                    record.player_id,
                    record.season,
                    record.week,
                    record.ts,
                    BigDecimal::from_f64(record.fantasy_pts).unwrap_or_default(),
                    record.is_game_over,
                    record.raw
                )
                .execute(&self.pool)
                .await
                .with_context(|| format!("Failed to store week points for player {}", record.player_id))?;
                
                stored_count += 1;
            }
        }
        
        info!("Stored {} new player week points for week {} (filtered out {} unmapped players)", stored_count, week, filtered_count);
        Ok(stored_count)
    }
    
    /// Run a complete season projections fetch and store cycle
    pub async fn run_season_projections_fetch(&self) -> Result<FetcherEvent> {
        info!("Starting season projections fetch");
        
        match self.fetch_season_projections().await {
            Ok(projections) => {
                let stored_count = self.store_season_projections(&projections).await?;
                
                Ok(FetcherEvent::ProjectionsUpdated {
                    count: stored_count,
                    timestamp: chrono::Utc::now(),
                })
            }
            Err(e) => {
                error!("Failed to fetch season projections: {}", e);
                Ok(FetcherEvent::FetchFailed {
                    endpoint: "season_projections".to_string(),
                    error: e.to_string(),
                    timestamp: chrono::Utc::now(),
                })
            }
        }
    }
    
    /// Run a complete player game stats fetch and store cycle
    pub async fn run_player_game_stats_fetch(&self, week: u32) -> Result<FetcherEvent> {
        info!("Starting player game stats fetch for week {}", week);
        
        match self.fetch_player_game_stats(week).await {
            Ok(stats) => {
                let stored_count = self.store_player_week_points(&stats, week).await?;
                
                Ok(FetcherEvent::PlayerWeekPointsUpdated {
                    count: stored_count,
                    week: week as i32,
                    timestamp: chrono::Utc::now(),
                })
            }
            Err(e) => {
                error!("Failed to fetch player game stats for week {}: {}", week, e);
                Ok(FetcherEvent::FetchFailed {
                    endpoint: format!("player_game_stats_week_{}", week),
                    error: e.to_string(),
                    timestamp: chrono::Utc::now(),
                })
            }
        }
    }
    
    /// Check if we should be polling for live stats
    pub fn should_poll_live_stats(&self) -> bool {
        self.config.is_live_window()
    }
    
    /// Get the current week
    pub fn get_current_week(&self) -> u32 {
        self.config.sportsdataio.week
    }
    
    /// Update the current week
    pub fn update_week(&mut self, week: u32) {
        self.config.sportsdataio.week = week;
    }
}
