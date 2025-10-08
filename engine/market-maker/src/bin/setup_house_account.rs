use anyhow::Result;
use sqlx::PgPool;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    info!("Setting up house account for market maker");
    
    // Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/waiver_exchange".to_string());
    
    let db_pool = PgPool::connect(&database_url).await?;
    info!("Connected to database");
    
    // Create house account in house_accounts table
    info!("Creating house account entry...");
    let house_account_id = sqlx::query!(
        r#"
        INSERT INTO house_accounts (account_type, display_name, currency_balance, is_active)
        VALUES ('house_market_maker', 'Market Maker Bot', 999999900, true)
        RETURNING id
        "#
    )
    .fetch_one(&db_pool)
    .await?;
    
    let account_id = house_account_id.id;
    info!("Created house account with ID: {}", account_id);
    
    // Create corresponding entry in accounts table
    info!("Creating regular account entry...");
    sqlx::query!(
        r#"
        INSERT INTO accounts (id, google_id, sleeper_user_id, currency_balance, fantasy_points, weekly_wins, created_at, last_updated)
        VALUES ($1, 'market_maker_bot', 'house_market_maker', 999999900, 0, 0, NOW(), NOW())
        ON CONFLICT (id) DO UPDATE SET 
            currency_balance = EXCLUDED.currency_balance,
            last_updated = NOW()
        "#,
        account_id
    )
    .execute(&db_pool)
    .await?;
    
    info!("Created regular account entry for ID: {}", account_id);
    
    // Verify the setup
    let account = sqlx::query!(
        "SELECT id, currency_balance FROM accounts WHERE id = $1",
        account_id
    )
    .fetch_one(&db_pool)
    .await?;
    
    let house_account = sqlx::query!(
        "SELECT id, account_type, display_name, currency_balance FROM house_accounts WHERE id = $1",
        account_id
    )
    .fetch_one(&db_pool)
    .await?;
    
    info!("✅ House account setup completed!");
    info!("  Account ID: {}", account.id);
    info!("  Balance: ${:.2}", account.currency_balance.unwrap_or(0) as f64 / 100.0);
    info!("  Type: {}", house_account.account_type);
    info!("  Display Name: {}", house_account.display_name);
    
    info!("📋 Next steps:");
    info!("  1. Add API key to order gateway: ak_market_maker_1234567890abcdef");
    info!("  2. Add API secret: sk_market_maker_abcdef1234567890");
    info!("  3. Update market maker config with these credentials");
    
    Ok(())
}
