use anyhow::Result;
use sqlx::PgPool;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Clearing duplicate RPE fair prices from database");
    
    // Get database URL from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/waiver_exchange".to_string());
    
    // Create database connection
    let pool = PgPool::connect(&database_url).await?;
    info!("Connected to database");
    
    // First, let's see how many records we have
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rpe_fair_prices")
        .fetch_one(&pool)
        .await?;
    info!("Total records in rpe_fair_prices: {}", total_count);
    
    // Count unique players
    let unique_players: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT player_id) FROM rpe_fair_prices")
        .fetch_one(&pool)
        .await?;
    info!("Unique players: {}", unique_players);
    
    // Show some examples of duplicates
    let duplicates = sqlx::query!(
        r#"
        SELECT player_id, COUNT(*) as count, MIN(ts) as first_ts, MAX(ts) as last_ts
        FROM rpe_fair_prices
        GROUP BY player_id
        HAVING COUNT(*) > 1
        ORDER BY COUNT(*) DESC
        LIMIT 10
        "#
    )
    .fetch_all(&pool)
    .await?;
    
    if !duplicates.is_empty() {
        warn!("Found {} players with duplicate records:", duplicates.len());
        for dup in &duplicates {
            warn!("  Player {}: {} records ({} to {})", 
                  dup.player_id, 
                  dup.count.unwrap_or(0), 
                  dup.first_ts.unwrap_or_default(),
                  dup.last_ts.unwrap_or_default());
        }
    }
    
    // Clear all records from rpe_fair_prices
    info!("Clearing all records from rpe_fair_prices table...");
    let deleted_count = sqlx::query("DELETE FROM rpe_fair_prices")
        .execute(&pool)
        .await?;
    
    info!("Deleted {} records from rpe_fair_prices", deleted_count.rows_affected());
    
    // Verify the table is empty
    let remaining_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rpe_fair_prices")
        .fetch_one(&pool)
        .await?;
    
    if remaining_count == 0 {
        info!("✅ Successfully cleared all duplicate records");
    } else {
        warn!("⚠️  {} records still remain in the table", remaining_count);
    }
    
    Ok(())
}
