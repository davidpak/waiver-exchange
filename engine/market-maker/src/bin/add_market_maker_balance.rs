use anyhow::Result;
use sqlx::PgPool;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    info!("Adding balance to market maker account");
    
    // Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/waiver_exchange".to_string());
    
    let db_pool = PgPool::connect(&database_url).await?;
    info!("Connected to database");
    
    // Find the market maker account
    let account = sqlx::query!(
        "SELECT id, currency_balance FROM accounts WHERE google_id = 'market_maker_bot'"
    )
    .fetch_one(&db_pool)
    .await?;
    
    let current_balance = account.currency_balance.unwrap_or(0);
    info!("Current balance: ${:.2}", current_balance as f64 / 100.0);
    
    // Add $50,000,000 (5 billion cents) to the account
    let additional_balance = 5_000_000_000i64; // $50 million
    let new_balance = current_balance + additional_balance;
    
    info!("Adding ${:.2} to account", additional_balance as f64 / 100.0);
    
    // Update the balance in both tables
    sqlx::query!(
        r#"
        UPDATE accounts 
        SET currency_balance = $1, last_updated = NOW()
        WHERE google_id = 'market_maker_bot'
        "#,
        new_balance
    )
    .execute(&db_pool)
    .await?;
    
    sqlx::query!(
        r#"
        UPDATE house_accounts 
        SET currency_balance = $1
        WHERE id = $2
        "#,
        new_balance,
        account.id
    )
    .execute(&db_pool)
    .await?;
    
    info!("✅ Balance updated successfully!");
    info!("  Account ID: {}", account.id);
    info!("  Previous balance: ${:.2}", current_balance as f64 / 100.0);
    info!("  New balance: ${:.2}", new_balance as f64 / 100.0);
    info!("  Added: ${:.2}", additional_balance as f64 / 100.0);
    
    Ok(())
}
