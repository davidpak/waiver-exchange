use anyhow::{Context, Result};
use sqlx::PgPool;
use std::env;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    info!("Setting up market maker positions for all symbols");
    
    // Get database URL from environment
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/waiver_exchange".to_string());
    
    // Connect to database
    let db_pool = PgPool::connect(&database_url)
        .await
        .context("Failed to connect to database")?;
    
    info!("Connected to database");
    
    // Market maker account ID (from setup-house-account)
    let account_id = 4i64;
    
    // First, let's check what data we have
    let fair_prices_count = sqlx::query!("SELECT COUNT(*) as count FROM rpe_fair_prices")
        .fetch_one(&db_pool)
        .await
        .context("Failed to count fair prices")?;
    
    let mapping_count = sqlx::query!("SELECT COUNT(*) as count FROM player_id_mapping")
        .fetch_one(&db_pool)
        .await
        .context("Failed to count player mappings")?;
    
    info!("Database status:");
    info!("  Fair prices in rpe_fair_prices: {}", fair_prices_count.count.unwrap_or(0));
    info!("  Player mappings: {}", mapping_count.count.unwrap_or(0));
    
    // Get all symbols that have fair prices
    let symbols_with_prices = sqlx::query!(
        r#"
        SELECT DISTINCT 
            pim.our_symbol_id,
            rfp.fair_cents
        FROM rpe_fair_prices rfp
        JOIN player_id_mapping pim ON rfp.player_id = pim.sportsdataio_player_id
        ORDER BY pim.our_symbol_id
        "#
    )
    .fetch_all(&db_pool)
    .await
    .context("Failed to fetch symbols with fair prices")?;
    
    info!("Found {} symbols with fair prices", symbols_with_prices.len());
    
    let mut positions_created = 0;
    
    for row in symbols_with_prices {
        let symbol_id = row.our_symbol_id as i64;
        let fair_price_cents = row.fair_cents;
        
        // Give the market maker substantial inventory for each symbol
        // 10000 basis points = 1 full share
        let quantity_bp = 1000000i64; // 100 shares per symbol
        let avg_cost_cents = fair_price_cents; // Use fair price as average cost
        
        info!("Creating position for symbol {}: {} bp ({} shares) at ${:.2} avg cost", 
              symbol_id, quantity_bp, quantity_bp / 10000, avg_cost_cents as f64 / 100.0);
        
        // Insert or update position
        sqlx::query!(
            r#"
            INSERT INTO positions (account_id, symbol_id, quantity, avg_cost, last_updated)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (account_id, symbol_id) 
            DO UPDATE SET 
                quantity = EXCLUDED.quantity,
                avg_cost = EXCLUDED.avg_cost,
                last_updated = NOW()
            "#,
            account_id,
            symbol_id,
            quantity_bp,
            avg_cost_cents
        )
        .execute(&db_pool)
        .await
        .context("Failed to create/update position")?;
        
        positions_created += 1;
    }
    
    info!("✅ Market maker positions setup completed!");
    info!("  Account ID: {}", account_id);
    info!("  Positions created: {}", positions_created);
    info!("  Quantity per symbol: 100 shares");
    info!("  Total inventory value: ~${:.2}", 
          positions_created as f64 * 100.0 * 150.0); // Rough estimate
    
    // Verify a few positions were created
    let sample_positions = sqlx::query!(
        "SELECT symbol_id, quantity, avg_cost FROM positions WHERE account_id = $1 LIMIT 5",
        account_id
    )
    .fetch_all(&db_pool)
    .await
    .context("Failed to verify positions")?;
    
    info!("📋 Sample positions created:");
    for pos in sample_positions {
        info!("  Symbol {}: {} bp ({} shares) at ${:.2}", 
              pos.symbol_id, 
              pos.quantity, 
              pos.quantity / 10000,
              pos.avg_cost as f64 / 100.0);
    }
    
    Ok(())
}
