use anyhow::{Context, Result};
use sqlx::PgPool;
use std::env;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🧹 Starting Player Week Points Cleanup");

    // Get database URL from environment
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/waiver_exchange".to_string());

    // Create database connection
    let pool = PgPool::connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    info!("✅ Connected to database");

    // Count total records before cleanup
    let total_before: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM player_week_points"
    )
    .fetch_one(&pool)
    .await
    .context("Failed to count records")?
    .unwrap_or(0);

    info!("📊 Total records before cleanup: {}", total_before);

    // Delete all but the most recent record for each (player_id, season, week) combination
    let deleted_count = sqlx::query!(
        r#"
        DELETE FROM player_week_points 
        WHERE (player_id, season, week, ts) NOT IN (
            SELECT DISTINCT ON (player_id, season, week) 
                player_id, season, week, ts
            FROM player_week_points 
            ORDER BY player_id, season, week, ts DESC
        )
        "#
    )
    .execute(&pool)
    .await
    .context("Failed to delete duplicate records")?;

    info!("🗑️  Deleted {} duplicate records", deleted_count.rows_affected());

    // Count total records after cleanup
    let total_after: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM player_week_points"
    )
    .fetch_one(&pool)
    .await
    .context("Failed to count records")?
    .unwrap_or(0);

    info!("📊 Total records after cleanup: {}", total_after);

    // Show some sample records to verify
    let sample_records = sqlx::query!(
        r#"
        SELECT player_id, season, week, ts, fantasy_pts, is_game_over
        FROM player_week_points 
        ORDER BY player_id, ts DESC
        LIMIT 10
        "#
    )
    .fetch_all(&pool)
    .await
    .context("Failed to fetch sample records")?;

    info!("📋 Sample records after cleanup:");
    for record in sample_records {
        info!("  Player {}: Week {} - {} pts (Game Over: {}) at {}", 
              record.player_id, 
              record.week, 
              record.fantasy_pts, 
              record.is_game_over.unwrap_or(false),
              record.ts);
    }

    info!("🎉 Cleanup completed successfully!");

    Ok(())
}
