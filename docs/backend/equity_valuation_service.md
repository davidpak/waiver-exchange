# Equity Valuation Service (EVS) Design

## Overview
The Equity Valuation Service provides real-time equity calculation and timeseries tracking for all accounts. It integrates with our existing tick-based architecture and event system.

## Architecture Integration

### Event Flow
```
SimulationClock (tick) 
    ↓
ExecutionManager (Trades → BookDeltas → OrderLifecycle → TickComplete)
    ↓
AccountService (balance updates)
    ↓
EquityValuationService (equity calculation)
    ↓
WebSocket (equity updates)
    ↓
Frontend (real-time chart)
```

### Core Components

#### 1. EquityValuationService
```rust
pub struct EquityValuationService {
    // Per-account state
    account_states: HashMap<i64, AccountEquityState>,
    
    // Dependencies
    account_service: Arc<AccountService>,
    snapshot_manager: Arc<SnapshotManager>,
    websocket_handler: Arc<WebSocketHandler>,
    
    // Storage
    db_pool: Arc<PgPool>,
    redis_client: RedisClient,
}

pub struct AccountEquityState {
    pub account_id: i64,
    pub cash_balance: i64,           // In cents
    pub positions: HashMap<u32, PositionState>, // symbol_id -> position
    pub current_equity: i64,         // In cents
    pub unrealized_pnl: i64,         // In cents
    pub realized_pnl: i64,           // In cents
    pub last_tick: u64,
    pub dirty: bool,                 // Needs recalculation
}

pub struct PositionState {
    pub symbol_id: u32,
    pub quantity: i64,               // Can be negative (short)
    pub avg_cost: i64,              // In cents
    pub current_price: i64,         // In cents (from snapshots)
}
```

#### 2. Event Handlers
```rust
impl EquityValuationService {
    // Called on every tick after TickComplete
    pub async fn on_tick_complete(&mut self, tick: u64, timestamp: chrono::DateTime<Utc>) -> Result<()> {
        for (account_id, state) in &mut self.account_states {
            if state.dirty {
                // Recalculate equity
                let new_equity = self.calculate_equity(*account_id, state).await?;
                state.current_equity = new_equity;
                state.dirty = false;
                
                // Emit equity update
                self.emit_equity_update(*account_id, tick, timestamp, state).await?;
            }
        }
        Ok(())
    }
    
    // Called when positions change (from AccountService)
    pub async fn on_position_change(&mut self, account_id: i64, symbol_id: u32) -> Result<()> {
        if let Some(state) = self.account_states.get_mut(&account_id) {
            // Reload position from database
            self.reload_position(account_id, symbol_id, state).await?;
            state.dirty = true;
        }
        Ok(())
    }
    
    // Called when balance changes (from AccountService)
    pub async fn on_balance_change(&mut self, account_id: i64, new_balance: i64) -> Result<()> {
        if let Some(state) = self.account_states.get_mut(&account_id) {
            state.cash_balance = new_balance;
            state.dirty = true;
        }
        Ok(())
    }
    
    // Called when prices change (from SnapshotManager)
    pub async fn on_price_change(&mut self, symbol_id: u32, new_price: i64) -> Result<()> {
        // Update all accounts holding this symbol
        for (account_id, state) in &mut self.account_states {
            if let Some(position) = state.positions.get_mut(&symbol_id) {
                let old_price = position.current_price;
                position.current_price = new_price;
                
                // Calculate P&L delta
                let pnl_delta = position.quantity * (new_price - old_price);
                state.unrealized_pnl += pnl_delta;
                state.dirty = true;
            }
        }
        Ok(())
    }
}
```

#### 3. Equity Calculation
```rust
impl EquityValuationService {
    async fn calculate_equity(&self, account_id: i64, state: &AccountEquityState) -> Result<i64> {
        let mut total_equity = state.cash_balance;
        
        // Add unrealized P&L from positions
        for (symbol_id, position) in &state.positions {
            if position.quantity != 0 {
                let position_value = position.quantity * position.current_price;
                total_equity += position_value;
            }
        }
        
        // Ensure non-negative equity
        Ok(total_equity.max(0))
    }
    
    async fn emit_equity_update(
        &self, 
        account_id: i64, 
        tick: u64, 
        timestamp: chrono::DateTime<Utc>,
        state: &AccountEquityState
    ) -> Result<()> {
        let equity_update = EquityUpdate {
            account_id,
            tick,
            timestamp: timestamp.timestamp_millis(),
            equity: state.current_equity,
            cash_balance: state.cash_balance,
            unrealized_pnl: state.unrealized_pnl,
            realized_pnl: state.realized_pnl,
        };
        
        // Send via WebSocket
        self.websocket_handler.broadcast_equity_update(equity_update).await?;
        
        // Store in timeseries (async)
        self.store_equity_point(equity_update).await?;
        
        Ok(())
    }
}
```

## Database Schema

### Timeseries Table
```sql
CREATE TABLE equity_timeseries (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT NOT NULL REFERENCES accounts(id),
    tick BIGINT NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    equity BIGINT NOT NULL,           -- In cents
    cash_balance BIGINT NOT NULL,     -- In cents
    unrealized_pnl BIGINT NOT NULL,   -- In cents
    realized_pnl BIGINT NOT NULL,     -- In cents
    created_at TIMESTAMP DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_equity_timeseries_account_tick ON equity_timeseries(account_id, tick);
CREATE INDEX idx_equity_timeseries_account_timestamp ON equity_timeseries(account_id, timestamp);
CREATE INDEX idx_equity_timeseries_timestamp ON equity_timeseries(timestamp);

-- Partition by day for better performance
-- (PostgreSQL 11+ supports native partitioning)
```

### Redis Cache
```rust
// Cache recent equity data for fast access
// Key: "equity:{account_id}:recent"
// TTL: 1 hour
// Value: JSON array of recent equity points
```

## WebSocket Integration

### Equity Update Message
```json
{
    "type": "equity_update",
    "account_id": 7,
    "tick": 1727224321,
    "timestamp": 1727224321000,
    "equity": 12873423,        // $1,287.34 in cents
    "cash_balance": 2183411,   // $2,183.41 in cents
    "unrealized_pnl": 41277,   // $412.77 in cents
    "realized_pnl": 9721       // $97.21 in cents
}
```

## REST API Endpoints

### Get Equity History
```rust
// GET /api/account/equity-history?account_id=7&range=1d&limit=1000
pub async fn get_equity_history(
    account_id: i64,
    range: String,  // "1d", "1w", "1m", "1y"
    limit: Option<usize>
) -> Result<EquityHistoryResponse> {
    // Query timeseries table with appropriate time range
    // Downsample for longer ranges (1m, 1h buckets)
}
```

## Performance Targets

- **Calculation**: O(1) per price change, ≤ 1ms per tick
- **WebSocket Latency**: ≤ 100ms p50 from TickComplete
- **Storage**: Async batch writes every second
- **Memory**: ~1KB per account state

## Integration Points

### 1. ExecutionManager Integration
```rust
// In ExecutionManager::tick()
impl ExecutionManager {
    async fn tick(&mut self, tick: u64) -> Result<()> {
        // ... existing tick logic ...
        
        // Emit TickComplete event
        self.emit_tick_complete(tick).await?;
        
        // Notify EVS
        self.equity_service.on_tick_complete(tick, Utc::now()).await?;
        
        Ok(())
    }
}
```

### 2. AccountService Integration
```rust
// In AccountService methods
impl AccountService {
    pub async fn update_balance(&self, account_id: i64, new_balance: i64) -> Result<()> {
        // ... existing balance update logic ...
        
        // Notify EVS
        self.equity_service.on_balance_change(account_id, new_balance).await?;
        
        Ok(())
    }
}
```

### 3. SnapshotManager Integration
```rust
// In SnapshotManager when prices change
impl SnapshotManager {
    pub async fn update_prices(&self, symbol_id: u32, new_price: i64) -> Result<()> {
        // ... existing price update logic ...
        
        // Notify EVS
        self.equity_service.on_price_change(symbol_id, new_price).await?;
        
        Ok(())
    }
}
```

## Frontend Integration

### Real-Time Chart Updates
```typescript
// In AccountSummary component
useEffect(() => {
    const handleEquityUpdate = (data: EquityUpdate) => {
        if (data.account_id === currentAccountId) {
            // Update chart with new equity point
            chartRef.current?.update({
                time: data.timestamp,
                value: data.equity / 100 // Convert cents to dollars
            });
            
            // Update summary display
            setCurrentEquity(data.equity);
            setDayChange(data.equity - openingEquity);
        }
    };
    
    websocketClient.subscribe('equity_update', handleEquityUpdate);
    
    return () => {
        websocketClient.unsubscribe('equity_update', handleEquityUpdate);
    };
}, [currentAccountId]);
```

## Benefits of This Approach

1. **Real-Time Accuracy**: Equity updates on every tick
2. **Performance**: O(1) updates, minimal memory usage
3. **Scalability**: Can handle thousands of accounts
4. **Integration**: Fits perfectly with existing architecture
5. **Storage Efficiency**: Only stores when values change
6. **Historical Data**: Full timeseries for analysis
7. **No Technical Debt**: Clean, maintainable design

## Implementation Priority

1. **Phase 1**: Core EVS with in-memory state
2. **Phase 2**: Database persistence and REST API
3. **Phase 3**: WebSocket integration and frontend updates
4. **Phase 4**: Performance optimization and monitoring
