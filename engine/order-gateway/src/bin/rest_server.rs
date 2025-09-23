//! REST API server for testing the new endpoints

use chrono::{Duration, Timelike, Utc};
use num_traits::FromPrimitive;
use order_gateway::cache::create_cache_manager;
use order_gateway::rest_api;
use persistence::config::SnapshotConfig;
use persistence::snapshot::SnapshotManager;
use player_registry::PlayerRegistry;
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting REST API server...");

    // Load player registry
    let mut registry = PlayerRegistry::new();
    registry.load_and_assign_symbols("data/players/season_projections_2025.json").await?;
    let registry = Arc::new(registry);

    info!("Loaded {} players into registry", registry.symbol_count());

    // Create database pool
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/waiver_exchange".to_string());
    let db_pool = PgPool::connect(&database_url).await?;
    let db_pool = Arc::new(db_pool);

    info!("Connected to database");

    // Create snapshot manager
    let snapshot_config = SnapshotConfig {
        interval: std::time::Duration::from_secs(60), // 1 minute intervals
        max_snapshots: 24,
        compress: true,
        snapshot_on_shutdown: true,
    };
    let snapshots_dir = PathBuf::from("data/snapshots");
    let snapshot_manager = SnapshotManager::new(snapshot_config, snapshots_dir)?;
    let snapshot_manager = Arc::new(snapshot_manager);
    info!("Created snapshot manager");

    // Create cache manager
    let cache = create_cache_manager().await?;
    let cache = Arc::new(cache);
    info!("Created cache manager");

    // Start daily equity snapshot scheduler
    start_daily_equity_scheduler(db_pool.clone());

    // Create routes
    let routes = rest_api::create_routes(registry, db_pool, snapshot_manager, cache);

    // Start server
    let port = 8081;
    info!("Starting REST API server on port {}", port);

    warp::serve(routes).run(([127, 0, 0, 1], port)).await;

    Ok(())
}

/// Start the daily equity snapshot scheduler
/// Runs daily at 4 PM EST (9 PM UTC) to create equity snapshots
fn start_daily_equity_scheduler(db_pool: Arc<PgPool>) {
    tokio::spawn(async move {
        info!("Starting daily equity snapshot scheduler");

        // Check every minute for the target time
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

        loop {
            interval.tick().await;

            let now = Utc::now();

            // Run at 4 PM EST (9 PM UTC) - market close time
            if now.hour() == 21 && now.minute() == 0 {
                info!("Running daily equity snapshot creation at market close");

                // Create snapshots for all accounts
                match create_daily_snapshots_internal(&db_pool).await {
                    Ok(snapshots_created) => {
                        info!("Successfully created {} daily equity snapshots", snapshots_created);
                    }
                    Err(e) => {
                        error!("Failed to create daily equity snapshots: {}", e);
                    }
                }

                // Wait a minute to avoid running multiple times in the same minute
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            }
        }
    });
}

/// Internal function to create daily snapshots (reused from rest_api.rs logic)
async fn create_daily_snapshots_internal(
    db_pool: &PgPool,
) -> Result<usize, Box<dyn std::error::Error>> {
    let today = Utc::now().date_naive();

    // Get all accounts
    let accounts = sqlx::query!("SELECT id FROM accounts").fetch_all(db_pool).await?;

    let mut snapshots_created = 0;

    for account in &accounts {
        let account_id = account.id;

        // Get account balance
        let balance =
            match sqlx::query!("SELECT currency_balance FROM accounts WHERE id = $1", account_id)
                .fetch_optional(db_pool)
                .await?
            {
                Some(row) => row.currency_balance.unwrap_or(0),
                None => {
                    tracing::warn!("Account {} not found", account_id);
                    continue;
                }
            };

        // Calculate total equity using existing logic
        let total_equity =
            calculate_total_equity_internal(db_pool, account_id, balance as u64).await;
        let position_value = if total_equity >= balance as u64 {
            total_equity - balance as u64
        } else {
            0 // Handle case where total equity is less than cash balance
        };

        // Get previous day's snapshot for comparison
        let previous_snapshot = sqlx::query!(
            "SELECT total_equity FROM daily_equity_snapshots 
             WHERE account_id = $1 AND date = $2",
            account_id,
            today - Duration::days(1)
        )
        .fetch_optional(db_pool)
        .await?;

        let (day_change, day_change_percent) = if let Some(prev) = previous_snapshot {
            let change = total_equity as i64 - prev.total_equity;
            let percent = if prev.total_equity > 0 {
                (change as f64 / prev.total_equity as f64) * 100.0
            } else {
                0.0
            };
            (
                change,
                sqlx::types::BigDecimal::from_f64(percent)
                    .unwrap_or(sqlx::types::BigDecimal::new(0.into(), 0)),
            )
        } else {
            (0, sqlx::types::BigDecimal::new(0.into(), 0)) // First day, no change
        };

        // Insert snapshot
        sqlx::query!(
            "INSERT INTO daily_equity_snapshots 
             (account_id, date, total_equity, cash_balance, position_value, day_change, day_change_percent)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (account_id, date) DO UPDATE SET
                 total_equity = EXCLUDED.total_equity,
                 cash_balance = EXCLUDED.cash_balance,
                 position_value = EXCLUDED.position_value,
                 day_change = EXCLUDED.day_change,
                 day_change_percent = EXCLUDED.day_change_percent",
            account_id,
            today,
            total_equity as i64,
            balance,
            position_value as i64,
            day_change,
            day_change_percent
        )
        .execute(db_pool)
        .await?;

        snapshots_created += 1;
        info!(
            "Created snapshot for account {}: equity={}, balance={}, position_value={}",
            account_id, total_equity, balance, position_value
        );
    }

    Ok(snapshots_created)
}

/// Calculate total equity for an account (internal version)
async fn calculate_total_equity_internal(
    db_pool: &PgPool,
    account_id: i64,
    cash_balance: u64,
) -> u64 {
    // Get all positions for this account
    match sqlx::query!(
        "SELECT symbol_id, quantity, avg_cost FROM positions WHERE account_id = $1",
        account_id
    )
    .fetch_all(db_pool)
    .await
    {
        Ok(positions) => {
            let mut total_position_value = 0i64; // Use i64 to handle negative values

            for position in positions {
                let quantity = position.quantity as i64; // Keep as i64 to handle negative quantities
                if quantity != 0 {
                    // Process both long and short positions
                    // Get current price for this symbol
                    let current_price =
                        get_current_price_internal(db_pool, position.symbol_id as i32).await;
                    let position_value = quantity * current_price as i64;
                    total_position_value += position_value;
                }
            }

            // Ensure we don't return negative values (cash balance should cover short positions)
            let final_equity = cash_balance as i64 + total_position_value;
            if final_equity < 0 {
                0 // Don't allow negative equity
            } else {
                final_equity as u64
            }
        }
        Err(_) => {
            // If we can't get positions, just return cash balance
            cash_balance
        }
    }
}

/// Get current price for a symbol (internal version)
async fn get_current_price_internal(db_pool: &PgPool, symbol_id: i32) -> u64 {
    // Try to get the most recent price from price_history
    match sqlx::query!(
        "SELECT close_price FROM price_history 
         WHERE symbol_id = $1 
         ORDER BY timestamp DESC 
         LIMIT 1",
        symbol_id
    )
    .fetch_optional(db_pool)
    .await
    {
        Ok(Some(row)) => row.close_price as u64,
        Ok(None) => {
            // Fallback to a default price if no history exists
            // This should rarely happen in production with bot activity
            1000 // $10.00 default price
        }
        Err(_) => {
            // Fallback on error
            1000 // $10.00 default price
        }
    }
}
