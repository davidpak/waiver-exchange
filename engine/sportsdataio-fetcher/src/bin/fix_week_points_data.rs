use anyhow::Result;
use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive};
use sportsdataio_fetcher::config::FetcherConfig;
use sqlx::PgPool;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    info!("Fixing player week points data to use standard FantasyPoints instead of PPR");
    
    // Load configuration
    let config = FetcherConfig::from_env()?;
    info!("Loaded configuration");
    
    // Create database connection
    let db_pool = PgPool::connect(&config.database.url).await?;
    info!("Connected to database");
    
    // Get all player week points records
    let records = sqlx::query!(
        "SELECT player_id, season, week, ts, fantasy_pts, raw FROM player_week_points ORDER BY player_id, season, week, ts"
    )
    .fetch_all(&db_pool)
    .await?;
    
    info!("Found {} player week points records", records.len());
    
    let mut updated_count = 0;
    let mut error_count = 0;
    
    for record in records {
        // Parse the raw JSON to get the correct FantasyPoints value
        if let Ok(raw_json) = serde_json::from_str::<serde_json::Value>(&record.raw.to_string()) {
            if let Some(fantasy_points) = raw_json.get("FantasyPoints").and_then(|v| v.as_f64()) {
                let current_pts = record.fantasy_pts.to_f64().unwrap_or(0.0);
                
                // Only update if the values are different (indicating PPR was stored)
                if (fantasy_points - current_pts).abs() > 0.01 {
                    info!(
                        "Updating player {} week {}: {} -> {} (standard FantasyPoints)",
                        record.player_id, record.week, current_pts, fantasy_points
                    );
                    
                    // Update the record with the correct standard FantasyPoints
                    let fantasy_points_bd = BigDecimal::from_f64(fantasy_points).unwrap_or_default();
                    match sqlx::query!(
                        "UPDATE player_week_points SET fantasy_pts = $1 WHERE player_id = $2 AND season = $3 AND week = $4 AND ts = $5",
                        fantasy_points_bd,
                        record.player_id,
                        record.season,
                        record.week,
                        record.ts
                    )
                    .execute(&db_pool)
                    .await
                    {
                        Ok(_) => updated_count += 1,
                        Err(e) => {
                            warn!("Failed to update player {}: {}", record.player_id, e);
                            error_count += 1;
                        }
                    }
                }
            }
        } else {
            warn!("Failed to parse raw JSON for player {}", record.player_id);
            error_count += 1;
        }
    }
    
    info!("✅ Data fix completed!");
    info!("  Updated records: {}", updated_count);
    info!("  Errors: {}", error_count);
    
    Ok(())
}
