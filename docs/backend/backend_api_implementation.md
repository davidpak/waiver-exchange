# Backend API Implementation Guide

**Owner**: Development Team  
**Status**: Implementation Ready  
**Scope**: Complete backend implementation for frontend API integration  
**Audience**: Backend developers implementing REST APIs and data services  

## 1. Overview

This document provides a comprehensive implementation guide for the backend APIs that power the Waiver Exchange frontend. The goal is to create a seamless, high-performance API layer that abstracts all backend complexity from frontend developers.

**Key Principles:**
- **Frontend only touches REST APIs** - no direct database or service access
- **1-second polling** for real-time updates (no WebSocket complexity)
- **Comprehensive error handling** with standardized responses
- **High performance** through caching and optimization
- **Complete data flow** from player data to price history

## 2. System Architecture

### 2.1 Data Sources
```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   JSON File     │    │   PostgreSQL     │    │     Redis       │
│ (Player Data)   │    │ (Account/Trades) │    │   (Caching)     │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                    REST API Layer                               │
│  (order-gateway + account-service + new endpoints)              │
└─────────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Frontend (React)                             │
│              (1-second polling for updates)                     │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Service Responsibilities
- **order-gateway**: Symbol info, price history, order placement
- **account-service**: Account data, positions, trades
- **ExecutionManager**: Price history recording, trade processing
- **PlayerRegistry**: Symbol ID assignment, player data management
- **Bot System**: Initial liquidity, market making, price discovery

## 3. Database Schema

### 3.1 New Tables

#### Price History Table
```sql
-- Migration: 002_add_price_history.sql
CREATE TABLE price_history (
    symbol_id INTEGER NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    open_price BIGINT NOT NULL,    -- Price in cents
    high_price BIGINT NOT NULL,    -- Price in cents
    low_price BIGINT NOT NULL,     -- Price in cents
    close_price BIGINT NOT NULL,   -- Price in cents
    volume BIGINT NOT NULL,        -- Number of shares
    PRIMARY KEY (symbol_id, timestamp)
);

-- Index for fast lookups
CREATE INDEX idx_price_history_symbol_time ON price_history(symbol_id, timestamp);
CREATE INDEX idx_price_history_timestamp ON price_history(timestamp);
```

#### Player Metadata Table (Optional - for caching)
```sql
-- Migration: 003_add_player_metadata.sql
CREATE TABLE player_metadata (
    player_id VARCHAR(50) PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    position VARCHAR(10) NOT NULL,
    team VARCHAR(10) NOT NULL,
    projected_points DECIMAL(10,2),
    rank INTEGER,
    symbol_id INTEGER,
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Index for symbol lookups
CREATE INDEX idx_player_metadata_symbol_id ON player_metadata(symbol_id);
CREATE INDEX idx_player_metadata_name ON player_metadata(name);
```

### 3.2 Existing Tables (Account Service)
- `accounts` - User account information
- `positions` - User holdings per symbol
- `trades` - Trade history
- `reservations` - Balance reservations for orders

## 4. Player Data Management

### 4.1 Symbol ID Assignment
The PlayerRegistry uses consistent hashing to assign symbol IDs:

```rust
// engine/player-registry/src/registry.rs
impl PlayerRegistry {
    /// Load player data and assign symbol IDs
    pub async fn load_and_assign_symbols<P: AsRef<Path>>(
        &mut self,
        file_path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Load player data from JSON
        let json_content = tokio::fs::read_to_string(&file_path).await?;
        let mut player_data: PlayerData = serde_json::from_str(&json_content)?;
        
        // 2. Assign symbol IDs using consistent hashing
        for player in &mut player_data.players {
            let symbol_id = ConsistentHasher::hash_to_symbol_id(
                &player.name,
                &player.position,
                &player.team,
                (player_data.players.len() * 2) as u32,
            );
            player.symbol_id = Some(symbol_id);
        }
        
        // 3. Save updated JSON with symbol IDs
        let updated_json = serde_json::to_string_pretty(&player_data)?;
        tokio::fs::write(&file_path, updated_json).await?;
        
        // 4. Create symbol mappings
        self.create_symbol_mappings(&player_data.players)?;
        
        Ok(())
    }
}
```

### 4.2 Player Data API
```rust
// GET /api/symbol/{symbolId}/info
// Returns player metadata from JSON file or database cache
```

## 5. Price History System

### 5.1 Price History Recording
**Architectural Decision:** Price history recording is handled by the AccountService, not the ExecutionManager.

**Rationale:**
- AccountService already has database access and handles trade settlement
- Keeps ExecutionManager focused on event processing and distribution
- Follows single responsibility principle
- No breaking changes to existing ExecutionManager instantiations

**Implementation Flow:**
```
Trade Event → ExecutionManager → AccountService.settle_trade() → AccountService.record_price_history()
```

The AccountService records price history when trades occur:

```rust
// engine/account-service/src/account.rs
impl AccountService {
    /// Record price history for a trade
    async fn record_price_history(&self, symbol_id: i32, price: i64, quantity: i64, timestamp: DateTime<Utc>) -> Result<(), Error> {
        // Get or create current candle
        let candle = self.get_or_create_candle(symbol_id, timestamp)?;
        
        // Update OHLC data
        candle.update_ohlc(price, quantity);
        
        // Store in PostgreSQL
        self.store_candle(candle)?;
        
        // Update Redis cache
        self.update_price_cache(symbol_id, candle)?;
        
        Ok(())
    }
    
    /// Get or create candle for timestamp
    fn get_or_create_candle(&self, symbol_id: u32, timestamp: DateTime<Utc>) -> Result<Candle, Error> {
        // Round timestamp to candle interval (5 minutes for 1D view)
        let candle_timestamp = self.round_to_candle_interval(timestamp);
        
        // Try to get existing candle
        if let Some(candle) = self.get_candle(symbol_id, candle_timestamp)? {
            return Ok(candle);
        }
        
        // Create new candle
        let candle = Candle::new(symbol_id, candle_timestamp, price, quantity);
        Ok(candle)
    }
}
```

### 5.2 Candle Data Structure
```rust
#[derive(Debug, Clone)]
pub struct Candle {
    pub symbol_id: u32,
    pub timestamp: DateTime<Utc>,
    pub open: u64,
    pub high: u64,
    pub low: u64,
    pub close: u64,
    pub volume: u64,
}

impl Candle {
    pub fn new(symbol_id: u32, timestamp: DateTime<Utc>, price: u64, quantity: u64) -> Self {
        Self {
            symbol_id,
            timestamp,
            open: price,
            high: price,
            low: price,
            close: price,
            volume: quantity,
        }
    }
    
    pub fn update_ohlc(&mut self, price: u64, quantity: u64) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        self.volume += quantity;
    }
}
```

## 6. Bot System Integration

### 6.1 Market Making Bots
Bots provide initial liquidity and price discovery:

```rust
// engine/bot-system/src/market_maker.rs
pub struct MarketMakerBot {
    symbol_id: u32,
    base_price: u64,
    spread: u64,
    order_size: u64,
}

impl MarketMakerBot {
    /// Provide initial liquidity for a symbol
    pub fn provide_liquidity(&self, tick: u64) -> Vec<Order> {
        let mut orders = Vec::new();
        
        // Place buy order below market
        let buy_price = self.base_price - self.spread;
        orders.push(Order::new_limit_buy(
            self.symbol_id,
            self.order_size,
            buy_price,
        ));
        
        // Place sell order above market
        let sell_price = self.base_price + self.spread;
        orders.push(Order::new_limit_sell(
            self.symbol_id,
            self.order_size,
            sell_price,
        ));
        
        orders
    }
    
    /// Update prices based on market activity
    pub fn update_prices(&mut self, last_trade_price: u64) {
        self.base_price = last_trade_price;
    }
}
```

### 6.2 Bot Integration Points
- **Initial Liquidity**: Bots place initial buy/sell orders
- **Price Discovery**: Bots adjust prices based on market activity
- **Market Making**: Bots provide continuous liquidity
- **Price History**: Bot activity creates initial price data

## 7. REST API Endpoints

### 7.1 Symbol Information API
```rust
// GET /api/symbol/{symbolId}/info
#[derive(Serialize)]
pub struct SymbolInfoResponse {
    pub sleeper_id: String,
    pub name: String,
    pub position: String,
    pub team: String,
    pub projected_points: f64,
    pub rank: Option<i32>,
    pub last_updated: String,
}
```

### 7.2 Price History API
```rust
// GET /api/price-history/{symbolId}?period=1d&interval=5m
#[derive(Serialize)]
pub struct PriceHistoryResponse {
    pub symbol_id: String,
    pub period: String,
    pub interval: String,
    pub candles: Vec<CandleData>,
}

#[derive(Serialize)]
pub struct CandleData {
    pub timestamp: String,
    pub open: u64,
    pub high: u64,
    pub low: u64,
    pub close: u64,
    pub volume: u64,
}
```

### 7.3 Current Price API
```rust
// GET /api/snapshot/current
// Returns current order book data from snapshots
```

### 7.4 Account Summary API
```rust
// GET /api/account/summary
#[derive(Serialize)]
pub struct AccountSummaryResponse {
    pub account_id: u64,
    pub currency_balance: u64,
    pub total_equity: u64,
    pub day_change: i64,
    pub day_change_percent: f64,
    pub buying_power: u64,
    pub last_updated: String,
}
```

### 7.5 Order Placement API
```rust
// POST /api/orders/place
#[derive(Deserialize)]
pub struct OrderPlaceRequest {
    pub symbol_id: String,
    pub side: String,
    pub quantity: u64,
    pub price: u64,
    pub order_type: String,
}

#[derive(Serialize)]
pub struct OrderPlaceResponse {
    pub order_id: String,
    pub status: String,
    pub message: Option<String>,
    pub timestamp: String,
}
```

### 7.6 Composite Endpoint (Recommended)
```rust
// GET /api/symbol/{symbolId}/complete
// Returns all symbol data in one request
#[derive(Serialize)]
pub struct SymbolCompleteResponse {
    pub info: SymbolInfoResponse,
    pub current_price: CurrentPriceData,
    pub price_history: PriceHistoryResponse,
    pub account_summary: AccountSummaryResponse,
}
```

## 8. Caching Strategy

### 8.1 Redis Cache Configuration
```rust
// Cache keys and TTLs
const PLAYER_METADATA_TTL: Duration = Duration::from_secs(86400); // 24h
const PRICE_HISTORY_TTL: Duration = Duration::from_secs(60);      // 1m
const CURRENT_PRICE_TTL: Duration = Duration::from_secs(1);       // 1s
const ACCOUNT_DATA_TTL: Duration = Duration::from_secs(30);       // 30s

// Cache key patterns
const PLAYER_METADATA_KEY: &str = "player:metadata:{}";
const PRICE_HISTORY_KEY: &str = "price:history:{}:{}:{}";
const CURRENT_PRICE_KEY: &str = "price:current:{}";
const ACCOUNT_SUMMARY_KEY: &str = "account:summary:{}";
```

## 9. Error Handling

### 9.1 Standardized Error Response
```rust
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
    pub timestamp: String,
}

#[derive(Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

// Error codes
pub enum ErrorCode {
    SymbolNotFound,
    InsufficientFunds,
    InvalidPrice,
    InvalidQuantity,
    OrderRejected,
    DatabaseError,
    CacheError,
    ValidationError,
}
```

### 9.2 Validation Rules
```rust
pub fn validate_order_request(request: &OrderPlaceRequest) -> Result<(), ApiError> {
    // Symbol ID validation
    if request.symbol_id.parse::<u32>().is_err() {
        return Err(ApiError::ValidationError("Invalid symbol ID format".to_string()));
    }
    
    // Price validation
    if request.price < 100 || request.price > 100000 {
        return Err(ApiError::ValidationError(
            "Price must be between 100 and 100000 cents".to_string()
        ));
    }
    
    // Quantity validation
    if request.quantity == 0 {
        return Err(ApiError::ValidationError("Quantity must be positive".to_string()));
    }
    
    // Side validation
    if request.side != "BUY" && request.side != "SELL" {
        return Err(ApiError::ValidationError("Side must be BUY or SELL".to_string()));
    }
    
    // Order type validation
    if request.order_type != "LIMIT" && request.order_type != "MARKET" {
        return Err(ApiError::ValidationError("Order type must be LIMIT or MARKET".to_string()));
    }
    
    Ok(())
}
```

## 10. Development Setup

### 10.1 Prerequisites
```bash
# Required services
- PostgreSQL 14+
- Redis 6+
- Rust 1.70+
- Node.js 18+ (for frontend development)
```

### 10.2 Database Setup
```bash
# Create database
createdb waiver_exchange

# Run migrations
psql waiver_exchange -f engine/account-service/migrations/001_initial_schema.sql
psql waiver_exchange -f engine/account-service/migrations/002_add_price_history.sql
psql waiver_exchange -f engine/account-service/migrations/003_add_player_metadata.sql
```

### 10.3 Redis Setup
```bash
# Start Redis
redis-server

# Test connection
redis-cli ping
```

### 10.4 Environment Configuration
```bash
# .env file
DATABASE_URL=postgresql://user:password@localhost/waiver_exchange
REDIS_URL=redis://localhost:6379
API_PORT=8080
LOG_LEVEL=info
```

### 10.5 Data Population
```bash
# Populate player data with symbol IDs
cargo run --bin player-registry -- --input data/players/season_projections_2025.json

# Start bot system for initial liquidity
cargo run --bin bot-system -- --symbols 764,256,128 --market-making
```

## 11. Implementation Checklist

### 11.1 Phase 1: Core Infrastructure
- [ ] Create database migrations
- [ ] Set up Redis caching
- [ ] Implement PlayerRegistry with symbol ID assignment
- [ ] Create basic REST endpoints
- [ ] Add error handling and validation

### 11.2 Phase 2: Data Services
- [ ] Implement price history recording in AccountService
- [ ] Add account summary calculations
- [ ] Implement order placement validation
- [ ] Add caching for all data types
- [ ] Create composite endpoints

### 11.3 Phase 3: Bot Integration
- [ ] Implement market maker bots
- [ ] Add initial liquidity provision
- [ ] Create price discovery mechanisms
- [ ] Test bot-generated price history
- [ ] Optimize bot performance

### 11.4 Phase 4: Testing & Optimization
- [ ] Add comprehensive unit tests
- [ ] Implement integration tests
- [ ] Add performance monitoring
- [ ] Optimize database queries
- [ ] Load test the system

### 11.5 Phase 5: Production Readiness
- [ ] Add health checks
- [ ] Implement logging and monitoring
- [ ] Create deployment scripts
- [ ] Add backup and recovery procedures
- [ ] Document operational procedures

## 12. Conclusion

This implementation guide provides a complete roadmap for building the backend APIs that power the Waiver Exchange frontend. The system is designed to be:

- **High Performance**: Caching, optimization, and efficient data structures
- **Reliable**: Comprehensive error handling and validation
- **Scalable**: Designed to handle growth in users and data
- **Maintainable**: Clear separation of concerns and well-documented code
- **Testable**: Comprehensive testing strategy and monitoring

The frontend will interact with clean, well-defined REST APIs that abstract all backend complexity, enabling rapid development and iteration.

---

## 13. Daily Equity Snapshots & History

### 13.1 Overview

Daily equity snapshots provide historical performance tracking for user accounts, enabling features like:
- Weekly/monthly/YTD performance charts
- Interactive equity history (hover to see daily values)
- Performance analytics and reporting

### 13.2 Database Schema

**Migration: `004_add_daily_equity_snapshots.sql`**

```sql
-- Daily equity snapshots for performance tracking
CREATE TABLE daily_equity_snapshots (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT NOT NULL REFERENCES accounts(id),
    date DATE NOT NULL,
    total_equity BIGINT NOT NULL, -- In cents
    cash_balance BIGINT NOT NULL, -- In cents
    position_value BIGINT NOT NULL, -- In cents
    day_change BIGINT NOT NULL, -- In cents (vs previous day)
    day_change_percent DECIMAL(10,4) NOT NULL, -- Percentage change
    created_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(account_id, date)
);

-- Indexes for performance
CREATE INDEX idx_daily_equity_account_date ON daily_equity_snapshots(account_id, date);
CREATE INDEX idx_daily_equity_date ON daily_equity_snapshots(date);
```

### 13.3 Scheduled Job Implementation

**AccountService Method: `create_daily_equity_snapshots`**

```rust
impl AccountService {
    /// Create daily equity snapshots for all accounts
    pub async fn create_daily_equity_snapshots(&self) -> Result<()> {
        let today = chrono::Utc::now().date_naive();
        
        // Get all accounts
        let accounts = sqlx::query!("SELECT id FROM accounts")
            .fetch_all(&self.db_pool)
            .await?;
        
        let mut snapshots = Vec::new();
        
        for account in accounts {
            let account_id = account.id;
            
            // Calculate current equity using existing logic
            let balance = self.get_balance(account_id).await?;
            let total_equity = self.calculate_total_equity(account_id, balance as u64).await;
            let position_value = total_equity - balance as u64;
            
            // Get previous day's snapshot for comparison
            let previous_snapshot = sqlx::query!(
                "SELECT total_equity FROM daily_equity_snapshots 
                 WHERE account_id = $1 AND date = $2",
                account_id,
                today - chrono::Duration::days(1)
            )
            .fetch_optional(&self.db_pool)
            .await?;
            
            let (day_change, day_change_percent) = if let Some(prev) = previous_snapshot {
                let change = total_equity as i64 - prev.total_equity;
                let percent = if prev.total_equity > 0 {
                    (change as f64 / prev.total_equity as f64) * 100.0
                } else {
                    0.0
                };
                (change, percent)
            } else {
                (0, 0.0) // First day, no change
            };
            
            snapshots.push((
                account_id,
                today,
                total_equity as i64,
                balance,
                position_value as i64,
                day_change,
                day_change_percent,
            ));
        }
        
        // Batch insert all snapshots
        for (account_id, date, total_equity, cash_balance, position_value, day_change, day_change_percent) in snapshots {
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
                account_id, date, total_equity, cash_balance, position_value, day_change, day_change_percent
            )
            .execute(&self.db_pool)
            .await?;
        }
        
        tracing::info!("Created daily equity snapshots for {} accounts", accounts.len());
        Ok(())
    }
    
    /// Calculate total equity for an account (reused from existing logic)
    async fn calculate_total_equity(&self, account_id: i64, cash_balance: u64) -> u64 {
        // Implementation matches existing calculate_total_equity function
        // ... (same logic as in rest_api.rs)
    }
}
```

### 13.4 Scheduling System

**Background Task in REST Server:**

```rust
// In rest_server.rs
async fn start_daily_equity_scheduler(account_service: Arc<AccountService>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60)); // 24 hours
        
        loop {
            interval.tick().await;
            
            // Run at 4 PM EST (market close)
            let now = chrono::Utc::now();
            if now.hour() == 21 && now.minute() == 0 { // 4 PM EST = 9 PM UTC
                if let Err(e) = account_service.create_daily_equity_snapshots().await {
                    tracing::error!("Failed to create daily equity snapshots: {}", e);
                }
            }
        }
    });
}
```

### 13.5 REST API Endpoints

**Equity History Endpoint:**

```rust
// Request/Response Models
#[derive(Debug, Serialize, Deserialize)]
pub struct EquityHistoryParams {
    pub account_id: i64,
    pub start_date: Option<String>, // YYYY-MM-DD format
    pub end_date: Option<String>,   // YYYY-MM-DD format
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EquityHistoryResponse {
    pub account_id: i64,
    pub snapshots: Vec<EquitySnapshot>,
    pub total_days: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EquitySnapshot {
    pub date: String, // YYYY-MM-DD
    pub total_equity: u64, // In cents
    pub cash_balance: u64, // In cents
    pub position_value: u64, // In cents
    pub day_change: i64, // In cents
    pub day_change_percent: f64, // Percentage
}

// Endpoint Implementation
pub async fn get_equity_history(
    params: EquityHistoryParams,
    db_pool: Arc<PgPool>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let start_date = params.start_date
        .unwrap_or_else(|| (chrono::Utc::now() - chrono::Duration::days(365)).format("%Y-%m-%d").to_string());
    let end_date = params.end_date
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    
    let rows = sqlx::query!(
        "SELECT date, total_equity, cash_balance, position_value, day_change, day_change_percent
         FROM daily_equity_snapshots 
         WHERE account_id = $1 AND date BETWEEN $2 AND $3
         ORDER BY date ASC",
        params.account_id,
        start_date,
        end_date
    )
    .fetch_all(&*db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to query equity history: {}", e);
        warp::reject::custom(NotFoundError(ErrorResponse {
            error: ErrorDetail {
                code: "DATABASE_ERROR".to_string(),
                message: "Failed to retrieve equity history".to_string(),
                details: Some(serde_json::Value::String(e.to_string())),
            },
            timestamp: chrono::Utc::now().to_rfc3339(),
        }))
    })?;
    
    let snapshots: Vec<EquitySnapshot> = rows
        .into_iter()
        .map(|row| EquitySnapshot {
            date: row.date.format("%Y-%m-%d").to_string(),
            total_equity: row.total_equity as u64,
            cash_balance: row.cash_balance as u64,
            position_value: row.position_value as u64,
            day_change: row.day_change,
            day_change_percent: row.day_change_percent,
        })
        .collect();
    
    let response = EquityHistoryResponse {
        account_id: params.account_id,
        snapshots,
        total_days: rows.len(),
    };
    
    Ok(warp::reply::json(&response))
}
```

### 13.6 Route Registration

```rust
// In create_routes function
let equity_history = warp::path("equity-history")
    .and(warp::get())
    .and(warp::query::<EquityHistoryParams>())
    .and(db_pool_filter.clone())
    .and_then(get_equity_history);
```

### 13.7 API Usage Examples

**Get 30-day equity history:**
```
GET /api/account/equity-history?account_id=1&start_date=2025-01-01&end_date=2025-01-31
```

**Response:**
```json
{
  "account_id": 1,
  "snapshots": [
    {
      "date": "2025-01-01",
      "total_equity": 100000,
      "cash_balance": 100000,
      "position_value": 0,
      "day_change": 0,
      "day_change_percent": 0.0
    },
    {
      "date": "2025-01-02",
      "total_equity": 105000,
      "cash_balance": 95000,
      "position_value": 10000,
      "day_change": 5000,
      "day_change_percent": 5.0
    }
  ],
  "total_days": 2
}
```

### 13.8 Implementation Complexity

**Daily Snapshots: MEDIUM**
- ✅ Database schema and migrations
- ✅ Reuse existing equity calculation logic
- ✅ Simple scheduled job with tokio::time::interval
- ⚠️ Handle edge cases (weekends, first-time accounts)
- ⚠️ Error handling for individual account failures

**Equity History Endpoint: LOW**
- ✅ Simple database query with date filtering
- ✅ Standard REST endpoint pattern
- ✅ Reuse existing error handling patterns

**Total Implementation Time: ~1 hour**

---

**Next Steps:**
1. Implement Phase 1 (Core Infrastructure)
2. Create bot system design document
3. Begin frontend development with API integration
4. Iterate based on testing and feedback
