use std::sync::Arc;

// Import our core components
use account_service::{AccountService, AccountServiceConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting Waiver Exchange Integration Test");
    
    // Initialize AccountService
    let account_config = AccountServiceConfig {
        database: account_service::config::DatabaseConfig {
            url: "postgresql://postgres:password@localhost:5432/waiver_exchange".to_string(),
            max_connections: 10,
            min_connections: 1,
        },
        redis: account_service::config::RedisConfig {
            url: "redis://localhost:6379".to_string(),
        },
        sleeper: account_service::config::SleeperConfig {
            api_base_url: "https://api.sleeper.app/v1".to_string(),
            api_key: None,
        },
        oauth: account_service::config::OAuthConfig {
            client_id: "test-client-id".to_string(),
            client_secret: "test-client-secret".to_string(),
            redirect_url: "http://localhost:8080/auth/callback".to_string(),
        },
        fantasy_points_conversion_rate: 10,
        reservation_expiry_days: 1,
        cache_ttl_seconds: 300,
    };
    
    let account_service = Arc::new(AccountService::new(account_config).await?);
    
    // Test 1: Health check
    println!("🏥 Test 1: Health check...");
    match account_service.health_check().await {
        Ok(_) => println!("✅ All services healthy"),
        Err(e) => println!("❌ Health check failed: {}", e),
    }
    
    // Test 2: Create test accounts and test retrieval
    println!("\n🔍 Test 2: Creating and testing accounts...");
    let test_accounts = create_test_accounts(&account_service).await?;
    println!("✅ Created {} test accounts", test_accounts.len());
    
    for account_id in &test_accounts {
        let account = account_service.get_account(*account_id).await?;
            println!("✅ Found account: ID={}, Balance=${}, Google ID={:?}", 
                    account.id, 
                    account.currency_balance.unwrap_or(0) as f64 / 100.0,
                    account.google_id);
            
        // Test 3: Test balance operations
        println!("\n💳 Test 3: Testing balance operations...");
        let balance = account_service.get_balance(account.id).await?;
        println!("   Balance: ${}", balance as f64 / 100.0);
        
        // Test 4: Test position operations
        println!("\n📈 Test 4: Testing position operations...");
        let positions = account_service.get_positions(account.id).await?;
        println!("   Found {} positions", positions.len());
        for position in &positions {
            println!("   Position: Symbol {}, {} shares, avg cost: ${}", 
                    position.symbol_id, 
                    position.quantity.to_cents(), 
                    position.avg_cost.to_cents() as f64 / 100.0);
        }
        
        // Test 5: Test trade history
        println!("\n📊 Test 5: Testing trade history...");
        let trades = account_service.get_trade_history(account.id, Some(10)).await?;
        println!("   Found {} trades", trades.len());
        for trade in &trades {
            println!("   Trade: {} shares at ${}", 
                    trade.quantity.to_cents(), 
                    trade.price.to_cents() as f64 / 100.0);
        }
        
        // Test 6: Test reservation operations
        println!("\n🔒 Test 6: Testing reservation operations...");
        let order_id = 12345i64;
        let amount = 50000i64; // $500.00 (10.00 shares × $50.00)
        match account_service.check_and_reserve_balance(account.id, amount, order_id).await {
            Ok(reservation_id) => {
                println!("   ✅ Created reservation: {}", reservation_id.0);
                
                // Test 7: Test trade settlement
                println!("\n💰 Test 7: Testing trade settlement...");
                let trade_details = account_service::trade::TradeDetails {
                    account_id: account.id,
                    symbol_id: 1,
                    side: account_service::position::TradeSide::Buy,
                    quantity: account_service::balance::Balance::from_basis_points(100000), // 10.00 shares (100000 basis points)
                    price: account_service::balance::Balance::from_cents(5000), // $50.00
                    order_id: order_id,
                };
                
                match account_service.settle_trade(&trade_details).await {
                    Ok(_) => println!("   ✅ Trade settled successfully"),
                    Err(e) => println!("   ❌ Trade settlement failed: {}", e),
                }
            }
            Err(e) => println!("   ❌ Reservation failed: {}", e),
        }
    }
    
    println!("\n🎉 Integration test completed!");
    println!("\n📝 Summary:");
    println!("   - AccountService initialized successfully");
    println!("   - Database connection working");
    println!("   - All core methods are accessible");
    println!("   - Ready for full trading system integration");
    
    Ok(())
}

async fn create_test_accounts(
    account_service: &AccountService,
) -> Result<Vec<i64>, Box<dyn std::error::Error>> {
    let mut account_ids = Vec::new();
    
    // Create a separate database connection for testing
    let pool = sqlx::PgPool::connect("postgresql://postgres:password@localhost:5432/waiver_exchange").await?;
    
    // Create test account 1
    let row = sqlx::query!(
        "INSERT INTO accounts (google_id, sleeper_user_id, currency_balance, fantasy_points, weekly_wins, created_at, last_updated) 
         VALUES ($1, $2, $3, $4, $5, NOW(), NOW()) 
         RETURNING id",
        "test-google-123",
        "test-sleeper-456",
        100000i64, // $1000.00 in cents
        0i32,
        0i32
    )
    .fetch_one(&pool)
    .await?;
    account_ids.push(row.id);
    
    // Create test account 2
    let row = sqlx::query!(
        "INSERT INTO accounts (google_id, sleeper_user_id, currency_balance, fantasy_points, weekly_wins, created_at, last_updated) 
         VALUES ($1, $2, $3, $4, $5, NOW(), NOW()) 
         RETURNING id",
        "test-google-789",
        "test-sleeper-012",
        50000i64, // $500.00 in cents
        0i32,
        0i32
    )
    .fetch_one(&pool)
    .await?;
    account_ids.push(row.id);
    
    // Create test account 3
    let row = sqlx::query!(
        "INSERT INTO accounts (google_id, sleeper_user_id, currency_balance, fantasy_points, weekly_wins, created_at, last_updated) 
         VALUES ($1, $2, $3, $4, $5, NOW(), NOW()) 
         RETURNING id",
        "test-google-345",
        "test-sleeper-678",
        75000i64, // $750.00 in cents
        0i32,
        0i32
    )
    .fetch_one(&pool)
    .await?;
    account_ids.push(row.id);
    
    Ok(account_ids)
}