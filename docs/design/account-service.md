> **Implementation Status:** Fully implemented. Account management, Google OAuth, Sleeper integration, balance tracking, position management, trade settlement, and reservation system all working. Redis is configured but token storage uses PostgreSQL. Weekly win bonus logic is simplified compared to this spec.

---

# AccountService Design Document

## 1. Overview

The `AccountService` is responsible for managing user accounts, balances, positions, and risk validation in the waiver-exchange system. It provides **deterministic admission** through read-only risk checks on the hot path and **authoritative settlement** by applying fills to cash/positions. It integrates with Google OAuth for authentication, Sleeper API for fantasy points conversion, and provides reservation-based balance management for limit orders.

**Key Features:**
- **Full OAuth 2.0 Flow**: Google OAuth with refresh token system
- **Session Management**: JWT access tokens (1 hour) + refresh tokens (30 days)
- **Fantasy points to currency conversion** ($10 per point)
- **Fractional share support** (4 decimal precision)
- **Balance reservations for limit orders** (7-day expiry)
- **Real-time risk validation**
- **Position tracking per symbol**
- **Auto-refresh tokens** for seamless user experience

### Role in the System

`AccountService` operates as an **embedded engine crate** that provides:

- **Balances & Positions**: Track per-account cash, per-symbol positions, and P&L
- **Risk & Eligibility**: Enforce limits before admission (cash sufficiency, position caps)
- **Settlement**: Apply trades and update positions based on ExecutionManager events
- **Reservations**: Manage balance reservations for limit orders with time-based expiry

### Core Responsibilities

- **Account Management**: Create accounts, get balances, manage user data
- **Risk Validation**: Check sufficient funds, position limits, exposure caps
- **Trade Settlement**: Update balances and positions after successful trades
- **Reservation Management**: Reserve balances for limit orders, handle expiry and cancellation
- **Authentication**: Full Google OAuth 2.0 flow with refresh tokens
- **Session Management**: JWT tokens, refresh token rotation, secure storage
- **Fantasy Integration**: Convert fantasy points to currency, update balances weekly

### Out of Scope

- Order matching or book management (Whistle)
- Event fan-out or persistence (ExecutionManager, PersistenceLayer)
- Real-time market data or external price feeds
- Multi-symbol coordination or cross-symbol risk

### Design Principles

1. **Non-blocking admission**: Whistle only does read lookups against local risk cache
2. **Deterministic timing**: Risk state changes only at tick boundaries
3. **Exactly-once settlement**: Trades applied idempotently using execution IDs
4. **Conservative reservation**: Reserve balances on admission, settle on fill
5. **Clear failure modes**: Every denial is explicit with specific reason codes

---

## 2. OAuth 2.0 Authentication System

### OAuth Flow Overview

The system implements a complete OAuth 2.0 flow with Google, providing secure authentication and session management with refresh tokens for seamless user experience.

**Session Management Strategy:**
- **Access Tokens (JWT)**: 1 hour lifetime, stored in frontend localStorage
- **Refresh Tokens**: 30 days lifetime, stored securely in Redis
- **Auto-refresh**: Seamless token refresh before expiry
- **User Experience**: Login once, stay logged in for 30 days

**OAuth Endpoints:**
- `GET /auth/google` - Initiate OAuth flow
- `POST /auth/callback` - Handle OAuth callback
- `POST /auth/refresh` - Refresh access token
- `POST /auth/logout` - Invalidate tokens

**Security Features:**
- JWT tokens with strong secret keys
- Refresh token rotation on each use
- Redis-based secure token storage
- CSRF protection with state parameters
- HTTPS-only in production

---

## 3. Database Schema

### Core Tables

```sql
-- Core account information
CREATE TABLE accounts (
    id BIGSERIAL PRIMARY KEY,
    google_id VARCHAR(255) UNIQUE NOT NULL,
    sleeper_user_id VARCHAR(255), -- Store user_id, not username
    sleeper_roster_id VARCHAR(255), -- Their roster in the chosen league
    sleeper_league_id VARCHAR(255), -- The ONE league they chose
    display_name VARCHAR(255),
    fantasy_points INTEGER DEFAULT 0,
    weekly_wins INTEGER DEFAULT 0, -- Track weekly wins for bonuses
    currency_balance BIGINT DEFAULT 0, -- Stored in cents
    created_at TIMESTAMP DEFAULT NOW(),
    last_updated TIMESTAMP DEFAULT NOW()
);

-- Position tracking per symbol
CREATE TABLE positions (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT REFERENCES accounts(id),
    symbol_id BIGINT NOT NULL,
    quantity BIGINT NOT NULL, -- Stored as basis points (1/10000th of a share)
    avg_cost BIGINT NOT NULL, -- Average cost in cents
    last_updated TIMESTAMP DEFAULT NOW(),
    UNIQUE(account_id, symbol_id)
);

-- Trade history
CREATE TABLE trades (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT REFERENCES accounts(id),
    symbol_id BIGINT NOT NULL,
    side VARCHAR(4) NOT NULL, -- 'BUY' or 'SELL'
    quantity BIGINT NOT NULL,
    price BIGINT NOT NULL, -- Price in cents
    timestamp TIMESTAMP DEFAULT NOW(),
    order_id BIGINT NOT NULL
);

-- Balance reservations for limit orders
CREATE TABLE reservations (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT REFERENCES accounts(id),
    amount BIGINT NOT NULL, -- Amount in cents
    order_id BIGINT NOT NULL,
    status VARCHAR(20) DEFAULT 'active', -- 'active', 'settled', 'expired', 'cancelled'
    created_at TIMESTAMP DEFAULT NOW(),
    expires_at TIMESTAMP NOT NULL
);

-- Indexes for performance
CREATE INDEX idx_accounts_google_id ON accounts(google_id);
CREATE INDEX idx_accounts_sleeper_user_id ON accounts(sleeper_user_id);
CREATE INDEX idx_accounts_sleeper_league_id ON accounts(sleeper_league_id);
CREATE INDEX idx_positions_account_id ON positions(account_id);
CREATE INDEX idx_trades_account_id ON trades(account_id);
CREATE INDEX idx_reservations_account_id ON reservations(account_id);
CREATE INDEX idx_reservations_expires_at ON reservations(expires_at);
```

### Data Types

- **Currency**: Stored in cents (e.g., $1.00 = 100 cents)
- **Quantities**: Stored as basis points (e.g., 1.0 share = 10000 basis points)
- **Timestamps**: UTC timestamps for all operations
- **Status**: Enum-like strings for reservation states

---

## 3. API Design

### Core AccountService

```rust
pub struct AccountService {
    db_pool: PgPool,
    redis_client: redis::Client,
    sleeper_client: SleeperClient,
    google_client: GoogleClient,
}

impl AccountService {
    // OAuth Authentication & Account Management
    pub async fn authenticate_with_google(&self, google_token: &str) -> Result<Account, Error>;
    pub async fn get_account_id_by_user_id(&self, user_id: &str) -> Result<i64, Error>;
    pub async fn create_account_from_oauth(&self, user_info: GoogleUserInfo) -> Result<Account, Error>;
    pub async fn link_sleeper_league(&self, account_id: u64, username: &str, season: &str) -> Result<Vec<LeagueOption>, Error>;
    pub async fn select_sleeper_league(&self, account_id: u64, league_id: &str) -> Result<(), Error>;
    pub async fn get_account(&self, account_id: u64) -> Result<Account, Error>;
    pub async fn update_fantasy_points_and_wins(&self, account_id: u64) -> Result<(), Error>;

    // Balance & Position Management
    pub async fn get_balance(&self, account_id: u64) -> Result<u64, Error>; // Returns cents
    pub async fn get_positions(&self, account_id: u64) -> Result<Vec<Position>, Error>;
    pub async fn get_position(&self, account_id: u64, symbol_id: u64) -> Result<Option<Position>, Error>;

    // Risk Validation & Reservations
    pub async fn check_and_reserve_balance(&self, account_id: u64, amount: u64, order_id: u64) -> Result<ReservationId, Error>;
    pub async fn settle_reserved_balance(&self, reservation_id: ReservationId, trade_details: TradeDetails) -> Result<(), Error>;
    pub async fn release_reservation(&self, reservation_id: ReservationId) -> Result<(), Error>;
    pub async fn cancel_order(&self, order_id: u64) -> Result<(), Error>;

    // Trade Settlement
    pub async fn settle_trade(&self, account_id: u64, symbol_id: u64, side: Side, quantity: u64, price: u64) -> Result<(), Error>;
    pub async fn get_trade_history(&self, account_id: u64, limit: Option<u32>) -> Result<Vec<Trade>, Error>;

    // Weekly Bonus System
    pub async fn calculate_weekly_bonus(&self, account_id: u64, week: u32) -> Result<u32, Error>;
    pub async fn apply_weekly_bonus(&self, account_id: u64, bonus_points: u32) -> Result<(), Error>;

    // Background Jobs
    pub async fn update_all_fantasy_points_and_wins(&self) -> Result<(), Error>;
    pub async fn cleanup_expired_reservations(&self) -> Result<(), Error>;
}
```

### Data Structures

```rust
#[derive(Debug, Clone)]
pub struct Account {
    pub id: u64,
    pub google_id: String,
    pub sleeper_user_id: Option<String>,
    pub sleeper_roster_id: Option<String>,
    pub sleeper_league_id: Option<String>,
    pub display_name: String,
    pub fantasy_points: u32,
    pub weekly_wins: u32,
    pub currency_balance: u64, // In cents
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleUserInfo {
    pub id: String,
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
    pub verified_email: bool,
}

#[derive(Debug, Clone)]
pub struct LeagueOption {
    pub id: String,
    pub name: String,
    pub season: String,
    pub roster_id: String,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub account_id: u64,
    pub symbol_id: u64,
    pub quantity: u64, // In basis points
    pub avg_cost: u64, // In cents
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct Trade {
    pub id: u64,
    pub account_id: u64,
    pub symbol_id: u64,
    pub side: Side,
    pub quantity: u64,
    pub price: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub order_id: u64,
}

#[derive(Debug, Clone)]
pub struct TradeDetails {
    pub account_id: u64,
    pub symbol_id: u64,
    pub side: Side,
    pub quantity: u64,
    pub price: u64,
    pub order_id: u64,
}

#[derive(Debug, Clone)]
pub struct ReservationId(pub u64);

#[derive(Debug, Clone)]
pub enum Side {
    Buy,
    Sell,
}
```

### Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum AccountServiceError {
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: u64, available: u64 },
    
    #[error("Account not found: {account_id}")]
    AccountNotFound { account_id: u64 },
    
    #[error("Reservation not found: {reservation_id}")]
    ReservationNotFound { reservation_id: u64 },
    
    #[error("Sleeper API error: {message}")]
    SleeperApiError { message: String },
    
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),
}
```

---

## 4. Integration Points

### OrderGateway Integration

```rust
// In OrderGateway::validate_order()
impl OrderGateway {
    async fn validate_order(&self, order: &Order) -> Result<(), ValidationError> {
        // Check if user has sufficient balance
        let required_amount = order.quantity * order.price;
        let reservation_id = self.account_service
            .check_and_reserve_balance(order.account_id, required_amount, order.id)
            .await?;
        
        // Store reservation_id with order for later settlement
        order.reservation_id = Some(reservation_id);
        Ok(())
    }
}
```

### ExecutionManager Integration

```rust
// In ExecutionManager::process_events()
impl ExecutionManager {
    async fn process_events(&mut self, events: Vec<DispatchEvent>) -> Result<(), Error> {
        for event in events {
            match event {
                DispatchEvent::TradeEvent(trade) => {
                    // Settle the trade with AccountService
                    self.account_service.settle_trade(
                        trade.buyer_account_id,
                        trade.symbol_id,
                        Side::Buy,
                        trade.quantity,
                        trade.price
                    ).await?;
                    
                    self.account_service.settle_trade(
                        trade.seller_account_id,
                        trade.symbol_id,
                        Side::Sell,
                        trade.quantity,
                        trade.price
                    ).await?;
                }
                DispatchEvent::OrderRejected(rejection) => {
                    // Release reservation if order was rejected
                    if let Some(reservation_id) = rejection.reservation_id {
                        self.account_service.release_reservation(reservation_id).await?;
                    }
                }
                // ... other events
            }
        }
        Ok(())
    }
}
```

### Whistle Integration

```rust
// In Whistle's process_message()
impl Whistle {
    fn process_message(&mut self, msg: InboundMsg, tick: TickId) -> Result<OrderHandle, RejectReason> {
        // Risk check via AccountService cache (non-blocking)
        if let Some(risk_cache) = &self.risk_cache {
            if !risk_cache.check_risk(msg.account_id, msg.amount) {
                return Err(RejectReason::InsufficientFunds);
            }
        }
        
        // Continue with order processing...
    }
}
```

---

## 5. Reservation System

### Reservation Lifecycle

```rust
// Reservation lifecycle:
1. User submits limit order → Reserve balance for 7 days
2. If order fills (fully or partially) → Settle reserved amount
3. If order expires → Release reservation, notify user
4. User can cancel order anytime → Release reservation
```

### Reservation Rules

- **Default expiry**: 7 days from creation
- **Extension policy**: Users can extend up to 30 days maximum
- **Partial fills**: Reserve remaining amount after partial fills
- **Cancellation**: Users can cancel anytime, releases reservation immediately
- **Cleanup**: Background job removes expired reservations

### Database Schema

```sql
-- Balance reservations for limit orders
CREATE TABLE reservations (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT REFERENCES accounts(id),
    amount BIGINT NOT NULL, -- Amount in cents
    order_id BIGINT NOT NULL,
    status VARCHAR(20) DEFAULT 'active', -- 'active', 'settled', 'expired', 'cancelled'
    created_at TIMESTAMP DEFAULT NOW(),
    expires_at TIMESTAMP NOT NULL
);
```

### Background Jobs

```rust
// Enhanced weekly fantasy points and wins update job
pub async fn update_fantasy_points_and_wins_job(account_service: &AccountService) -> Result<(), Error> {
    let accounts = account_service.get_all_accounts_with_sleeper().await?;
    
    for account in accounts {
        if let (Some(_), Some(_)) = (&account.sleeper_league_id, &account.sleeper_roster_id) {
            match account_service.update_fantasy_points_and_wins(account.id).await {
                Ok(_) => {
                    tracing::info!("Updated fantasy points and wins for account {}", account.id);
                }
                Err(e) => {
                    tracing::error!("Failed to update fantasy points for account {}: {}", account.id, e);
                }
            }
        }
    }
    
    Ok(())
}

// Reservation cleanup job
pub async fn cleanup_reservations_job(account_service: &AccountService) -> Result<(), Error> {
    let expired_reservations = account_service.get_expired_reservations().await?;
    
    for reservation in expired_reservations {
        account_service.release_reservation(reservation.id).await?;
        
        // Notify user about expired order
        send_expiration_notification(reservation.account_id, reservation.order_id).await?;
    }
    
    Ok(())
}
```

---

## 6. Fantasy Points Integration

### Conversion Strategy

- **Base Rate**: $10 per fantasy point
- **Weekly Win Bonus**: 10% of weekly fantasy points earned for wins
- **Example**: Josh Allen (110 pts/week) = $1,100/week base
- **With weekly wins**: Additional $110 bonus per win
- **Over 17 weeks**: $18,700 base + potential $1,870 in win bonuses

### Sleeper API Integration

```rust
// Enhanced OAuth flow with league selection
1. User clicks "Login with Google"
2. Redirect to Google OAuth
3. Get user info
4. User provides Sleeper username
5. Fetch user's leagues for current season
6. If multiple leagues: User selects ONE league
7. If single league: Auto-select
8. Store sleeper_user_id, sleeper_roster_id, sleeper_league_id
9. Convert fantasy points + weekly wins to currency
10. Create/update account
```

### Weekly Updates

- **Sunday 9pm PT**: After Sunday games
- **Monday 9pm PT**: After Monday Night Football  
- **Thursday 9pm PT**: After Thursday Night Football
- **Background job**: Fetch updated fantasy points, update balances

### Implementation

```rust
impl AccountService {
    async fn link_sleeper_league(&self, account_id: u64, username: &str, season: &str) -> Result<Vec<LeagueOption>, Error> {
        // Step A: Resolve username to user_id
        let user_id = self.sleeper_client.get_user_id(username).await?;
        
        // Step B: Get user's leagues for the season
        let leagues = self.sleeper_client.get_user_leagues(&user_id, season).await?;
        
        // Convert to LeagueOption format
        let league_options = leagues.into_iter().map(|league| LeagueOption {
            id: league.id,
            name: league.name,
            season: season.to_string(),
            roster_id: league.roster_id,
        }).collect();
        
        Ok(league_options)
    }
    
    async fn update_fantasy_points_and_wins(&self, account_id: u64) -> Result<(), Error> {
        let account = self.get_account(account_id).await?;
        
        if let (Some(league_id), Some(roster_id)) = (&account.sleeper_league_id, &account.sleeper_roster_id) {
            // Get season total fantasy points
            let season_points = self.sleeper_client.get_season_points(league_id, roster_id).await?;
            
            // Calculate weekly bonuses for all completed weeks
            let mut total_bonus = 0;
            for week in 1..=get_current_week() {
                let bonus = self.calculate_weekly_bonus(account_id, week).await?;
                total_bonus += bonus;
            }
            
            // Convert to currency: (season_points + bonuses) * $10
            let total_currency = (season_points + total_bonus) * 10;
            
            // Update account
            self.update_account_balance(account_id, total_currency).await?;
        }
        
        Ok(())
    }
    
    async fn calculate_weekly_bonus(&self, account_id: u64, week: u32) -> Result<u32, Error> {
        let account = self.get_account(account_id).await?;
        
        if let (Some(league_id), Some(roster_id)) = (&account.sleeper_league_id, &account.sleeper_roster_id) {
            // Get matchup for this week
            let matchup = self.sleeper_client.get_weekly_matchup(league_id, week).await?;
            
            // Find user's team and opponent
            let my_team = matchup.iter().find(|t| t.roster_id == *roster_id)?;
            let opponent = matchup.iter().find(|t| t.matchup_id == my_team.matchup_id && t.roster_id != *roster_id)?;
            
            // Check if user won
            if my_team.points > opponent.points {
                // Bonus: 10% of weekly fantasy points earned
                let bonus = (my_team.points * 0.1) as u32;
                Ok(bonus)
            } else {
                Ok(0)
            }
        } else {
            Ok(0)
        }
    }
}
```

---

## 7. Fractional Shares

### Implementation

```rust
// Current system uses u32 for quantities
order.qty_open: u32  // Whole shares only

// With fractional shares, change to fixed-point arithmetic
order.qty_open: u64  // Represent as basis points (1/10000th of a share)
// 1.0 share = 10000 basis points
// 0.5 shares = 5000 basis points
// 0.25 shares = 2500 basis points
```

### Benefits

- **More accessible**: Users can buy $50 worth of Josh Allen instead of needing $350
- **Better price discovery**: More granular trading
- **Realistic**: Matches how people actually invest
- **Easy to implement**: Just change quantity representation

### Example Calculations

```rust
// Example: Josh Allen at $350/share
$50 order = 0.1429 shares (50 ÷ 350)
// Rounded to 4 decimals = 0.1429 shares
// User gets exactly $50 worth, no leftover
```

---

## 8. Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_balance_reservation() {
        let account_service = setup_test_account_service().await;
        
        // Create test account with $100 balance
        let account_id = create_test_account(&account_service, 10000).await; // $100 in cents
        
        // Reserve $50
        let reservation_id = account_service
            .check_and_reserve_balance(account_id, 5000, 123)
            .await
            .unwrap();
        
        // Check balance is still $100 (reserved, not deducted)
        let balance = account_service.get_balance(account_id).await.unwrap();
        assert_eq!(balance, 10000);
        
        // Settle the reservation
        let trade_details = TradeDetails {
            account_id,
            symbol_id: 1,
            side: Side::Buy,
            quantity: 5000,
            price: 100,
            order_id: 123,
        };
        
        account_service.settle_reserved_balance(reservation_id, trade_details).await.unwrap();
        
        // Check balance is now $50
        let balance = account_service.get_balance(account_id).await.unwrap();
        assert_eq!(balance, 5000);
    }

    #[tokio::test]
    async fn test_insufficient_balance_rejection() {
        let account_service = setup_test_account_service().await;
        let account_id = create_test_account(&account_service, 1000).await; // $10 in cents
        
        // Try to reserve $50 (should fail)
        let result = account_service
            .check_and_reserve_balance(account_id, 5000, 123)
            .await;
        
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fractional_share_calculations() {
        let account_service = setup_test_account_service().await;
        
        // Test fractional share precision
        let quantity = 1429; // 0.1429 shares in basis points
        let price = 35000; // $350 in cents
        
        // Should calculate exactly $50 worth
        let total_cost = (quantity * price) / 10000;
        assert_eq!(total_cost, 5000); // $50 in cents
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_end_to_end_order_flow() {
    // Setup test environment
    let (order_gateway, execution_manager, account_service) = setup_test_system().await;
    
    // Create test account
    let account_id = create_test_account(&account_service, 10000).await; // $100
    
    // Submit order for $50 worth of shares
    let order = Order {
        id: 123,
        account_id,
        symbol_id: 1,
        side: Side::Buy,
        quantity: 1429, // 0.1429 shares
        price: 35000, // $350 per share
        order_type: OrderType::Limit,
    };
    
    // Validate order (should reserve balance)
    order_gateway.validate_order(&order).await.unwrap();
    
    // Check balance is reserved
    let balance = account_service.get_balance(account_id).await.unwrap();
    assert_eq!(balance, 10000); // Still $100, but $50 is reserved
    
    // Simulate order execution
    let trade_event = DispatchEvent::TradeEvent(TradeEvent {
        buyer_account_id: account_id,
        seller_account_id: 999, // Test seller
        symbol_id: 1,
        quantity: 1429,
        price: 35000,
        timestamp: chrono::Utc::now(),
    });
    
    execution_manager.process_events(vec![trade_event]).await.unwrap();
    
    // Check balance is now $50
    let balance = account_service.get_balance(account_id).await.unwrap();
    assert_eq!(balance, 5000);
    
    // Check position was created
    let position = account_service.get_position(account_id, 1).await.unwrap().unwrap();
    assert_eq!(position.quantity, 1429);
}
```

---

## 9. Configuration

### Environment Variables

```rust
// Environment variables
DATABASE_URL=postgresql://user:pass@localhost/waiver_exchange
REDIS_URL=redis://localhost:6379
GOOGLE_CLIENT_ID=your_google_client_id
GOOGLE_CLIENT_SECRET=your_google_client_secret
SLEEPER_API_BASE_URL=https://api.sleeper.app/v1
FANTASY_POINTS_CONVERSION_RATE=10
RESERVATION_EXPIRY_DAYS=7
```

### Configuration Structure

```rust
#[derive(Debug, Clone)]
pub struct AccountServiceConfig {
    pub database_url: String,
    pub redis_url: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub sleeper_api_base_url: String,
    pub fantasy_points_conversion_rate: u32,
    pub reservation_expiry_days: u32,
    pub cache_ttl_seconds: u32,
}
```

---

## 10. Performance Considerations

### Caching Strategy

```rust
// Redis cache keys with TTL
"account:{id}:balance" -> balance (5 seconds)
"account:{id}:positions" -> positions (10 seconds)  
"account:{id}:risk_check" -> risk result (1 second)
"account:{id}:daily_pnl" -> daily P&L (1 hour)
```

### Database Optimization

- **Indexes**: On frequently queried columns (account_id, symbol_id, expires_at)
- **Connection pooling**: Use PgPool for efficient database connections
- **Query optimization**: Use prepared statements and batch operations
- **Read replicas**: For read-heavy operations like balance checks

### Memory Management

- **Connection pooling**: Reuse database and Redis connections
- **Batch operations**: Group multiple operations together
- **Async operations**: Non-blocking I/O for all external calls
- **Error handling**: Graceful degradation when services are unavailable

---

## 11. Monitoring & Observability

### Metrics

```rust
// Key metrics to track
- account_balance_checks_total
- reservation_created_total
- reservation_settled_total
- reservation_expired_total
- trade_settled_total
- fantasy_points_updated_total
- weekly_wins_calculated_total
- sleeper_api_calls_total
- database_connection_errors_total
```

### Logging

```rust
// Structured logging
tracing::info!(
    account_id = %account_id,
    amount = %amount,
    order_id = %order_id,
    "Balance reserved for order"
);

tracing::error!(
    account_id = %account_id,
    error = %error,
    "Failed to update fantasy points"
);
```

### Health Checks

```rust
impl AccountService {
    pub async fn health_check(&self) -> Result<HealthStatus, Error> {
        // Check database connectivity
        self.db_pool.execute("SELECT 1").await?;
        
        // Check Redis connectivity
        self.redis_client.get_connection()?.ping()?;
        
        // Check Sleeper API
        self.sleeper_client.health_check().await?;
        
        Ok(HealthStatus::Healthy)
    }
}
```

---

## 12. Implementation Plan

### Phase 1: Core System
1. **Database Schema**: Create tables and indexes
2. **Basic AccountService**: Core struct and database operations
3. **Fractional Share Support**: Update quantity representation
4. **Simple Risk Checks**: Sufficient balance validation

### Phase 2: Authentication & Integration
1. **Google OAuth Flow**: User authentication
2. **Sleeper Integration**: League selection, fantasy points lookup and conversion
3. **Weekly Win Bonus System**: Track and reward weekly wins
4. **Background Scheduler**: Weekly balance and win updates
5. **Reservation System**: Balance reservations and cleanup

### Phase 3: Advanced Features
1. **Position Tracking**: Multi-symbol position management
2. **Advanced Risk Rules**: Position limits, exposure caps
3. **Performance Optimization**: Caching, connection pooling
4. **Monitoring**: Metrics, logging, health checks

### Phase 4: Production Readiness
1. **Comprehensive Testing**: Unit, integration, and end-to-end tests
2. **Error Handling**: Graceful degradation and recovery
3. **Documentation**: API documentation and operational guides
4. **Deployment**: Production configuration and monitoring

---

## 13. Invariants

### System Invariants

1. **Balance Consistency**: `available_cash = cash − Σ(active_cash_reservations) ≥ 0`
2. **Position Consistency**: `available_qty[symbol] = position_qty − Σ(active_qty_reservations[symbol]) ≥ 0`
3. **Reservation Integrity**: Sum of reservations equals sum of admissible open quantities * prices
4. **Idempotent Settlement**: Applying the same trade twice does not change balances/positions
5. **Snapshot Consistency**: Snapshot → restore → replay produces identical balances/positions/P&L

### Operational Invariants

1. **Non-blocking Admission**: Risk checks never block the hot path
2. **Deterministic Timing**: Risk state changes only at tick boundaries
3. **Exactly-once Settlement**: Trades applied exactly once using execution IDs
4. **Reservation Lifecycle**: All reservations have defined expiry and cleanup
5. **Data Consistency**: Database and cache remain consistent across operations

---

## 14. Failure Modes & Recovery

### Failure Scenarios

| Scenario | Behavior | Recovery |
|----------|----------|----------|
| Database unavailable | Reject new operations, log errors | Retry with exponential backoff |
| Redis unavailable | Fall back to database, log warnings | Cache rebuild on recovery |
| Sleeper API down | Skip fantasy point updates | Retry on next scheduled update |
| Reservation expiry | Release expired reservations | Background cleanup job |
| Partial trade settlement | Log error, retry settlement | Manual reconciliation if needed |

### Error Handling

```rust
// Graceful degradation
impl AccountService {
    async fn check_risk_with_fallback(&self, account_id: u64, amount: u64) -> Result<bool, Error> {
        // Try cache first
        if let Ok(cached_result) = self.redis_client.get_risk_check(account_id).await {
            return Ok(cached_result);
        }
        
        // Fall back to database
        if let Ok(db_result) = self.database_check_risk(account_id, amount).await {
            return Ok(db_result);
        }
        
        // If all else fails, reject for safety
        Err(AccountServiceError::RiskUnavailable)
    }
}
```

---

## 15. Security Considerations

### Data Protection

- **Encryption**: All sensitive data encrypted at rest and in transit
- **Access Control**: Role-based access to account data
- **Audit Logging**: All account operations logged for compliance
- **Data Retention**: Configurable retention policies for trade history

### API Security

- **Authentication**: Google OAuth for user authentication
- **Authorization**: Account-based access control
- **Rate Limiting**: Prevent abuse of account operations
- **Input Validation**: Sanitize all user inputs

### Operational Security

- **Connection Security**: TLS for all external connections
- **Credential Management**: Secure storage of API keys and secrets
- **Monitoring**: Real-time monitoring for suspicious activity
- **Incident Response**: Defined procedures for security incidents

---

**Key Principle**: AccountService provides deterministic, non-blocking risk validation and authoritative settlement while maintaining data consistency and operational reliability.
