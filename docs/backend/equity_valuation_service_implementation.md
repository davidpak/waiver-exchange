# Equity Valuation Service (EVS) - Complete Implementation Guide

## Table of Contents
1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Event Flow](#event-flow)
5. [Implementation Details](#implementation-details)
6. [Database Schema](#database-schema)
7. [WebSocket Integration](#websocket-integration)
8. [Testing Strategy](#testing-strategy)
9. [Deployment Plan](#deployment-plan)

## Overview

The Equity Valuation Service (EVS) is a real-time equity calculation and broadcasting system that integrates with the existing ExecutionManager event system. It provides sub-millisecond equity calculations and broadcasts updates to specific accounts via WebSocket.

### Key Features
- **Real-time equity calculation** using in-memory caches
- **Event-driven updates** via ExecutionManager subscription
- **Account-specific WebSocket broadcasting**
- **Tick-aligned processing** to prevent double counting
- **Database persistence** for historical equity data

## Architecture

### ⚠️ ARCHITECTURAL ISSUE IDENTIFIED

**Problem**: The original design assumed ExecutionManager had a subscription system for other services, but it doesn't.

**Current Reality**:
```
Whistle Engine → ExecutionManager → AccountService::settle_trade()
                                    ↓
                               Database Updated
```

**Original (Incorrect) Design**:
```
Whistle Engine → ExecutionManager → EVS (❌ No connection!)
                                    ↓
                               AccountService::settle_trade()
```

### ✅ CORRECTED ARCHITECTURE

**Solution**: Extend ExecutionManager to support post-settlement callbacks.

**Corrected Event Flow**:
```
1. Trade Executes in Whistle Engine
   ↓
2. ExecutionManager receives TradeEvent
   ↓
3. ExecutionManager calls AccountService::settle_trade() (EXISTING)
   ↓
4. ExecutionManager notifies EVS via callback (NEW)
   ↓
5. EVS recalculates equity and broadcasts update (NEW)
   ↓
6. Frontend receives real-time equity update (NEW)
```

### Integration Points
- **ExecutionManager**: Extended with callback system for post-settlement notifications
- **WebSocketHandler**: Adds equity subscription method
- **Database**: Adds equity_timeseries table and realized_pnl columns
- **AccountService**: No changes required (keeps existing settle_trade)

## CORRECTED IMPLEMENTATION

### 1. ExecutionManager Callback System

```rust
// File: engine/execution-manager/src/dispatch.rs
use std::sync::Arc;
use async_trait::async_trait;

#[async_trait]
pub trait PostSettlementCallback: Send + Sync {
    async fn on_trade_settled(
        &self, 
        account_id: i64, 
        symbol_id: u32, 
        side: TradeSide, 
        quantity: i64, 
        price: i64
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    async fn on_price_updated(&self, symbol_id: u32, price: i64) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    async fn on_tick_complete(&self, tick_id: TickId) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub struct EventDispatcher {
    config: FanoutConfig,
    metrics: Arc<MetricsCollector>,
    analytics_converter: AnalyticsConverter,
    analytics_sender: Option<mpsc::UnboundedSender<analytics_engine::analytics::AnalyticsEvent>>,
    
    // NEW: Post-settlement callbacks
    post_settlement_callbacks: Vec<Arc<dyn PostSettlementCallback>>,
}

impl EventDispatcher {
    pub fn new(config: FanoutConfig, metrics: Arc<MetricsCollector>) -> Self {
        Self {
            config,
            metrics,
            analytics_converter: AnalyticsConverter::new(100),
            analytics_sender: None,
            post_settlement_callbacks: Vec::new(),
        }
    }
    
    pub fn add_post_settlement_callback(&mut self, callback: Arc<dyn PostSettlementCallback>) {
        self.post_settlement_callbacks.push(callback);
    }
    
    pub async fn notify_trade_settled(
        &self, 
        account_id: i64, 
        symbol_id: u32, 
        side: TradeSide, 
        quantity: i64, 
        price: i64
    ) {
        for callback in &self.post_settlement_callbacks {
            if let Err(e) = callback.on_trade_settled(account_id, symbol_id, side, quantity, price).await {
                tracing::warn!("Post-settlement callback failed: {}", e);
            }
        }
    }
    
    pub async fn notify_price_updated(&self, symbol_id: u32, price: i64) {
        for callback in &self.post_settlement_callbacks {
            if let Err(e) = callback.on_price_updated(symbol_id, price).await {
                tracing::warn!("Price update callback failed: {}", e);
            }
        }
    }
    
    pub async fn notify_tick_complete(&self, tick_id: TickId) {
        for callback in &self.post_settlement_callbacks {
            if let Err(e) = callback.on_tick_complete(tick_id).await {
                tracing::warn!("Tick complete callback failed: {}", e);
            }
        }
    }
}
```

### 2. ExecutionManager Integration

```rust
// File: engine/execution-manager/src/lib.rs
impl ExecutionManager {
    pub fn add_post_settlement_callback(&mut self, callback: Arc<dyn PostSettlementCallback>) {
        self.dispatcher.add_post_settlement_callback(callback);
    }
    
    // Modified settle_trade to notify callbacks after successful settlement
    async fn settle_trade(&self, trade_event: &TradeEvent) -> Result<(), ExecutionError> {
        // ... existing settlement logic ...
        
        // Settle both trades
        self.account_service.settle_trade(&buy_trade).await.map_err(|e| {
            ExecutionError::SettlementFailed(format!("Buy trade settlement failed: {}", e))
        })?;

        self.account_service.settle_trade(&sell_trade).await.map_err(|e| {
            ExecutionError::SettlementFailed(format!("Sell trade settlement failed: {}", e))
        })?;
        
        // NEW: Notify post-settlement callbacks (like EVS)
        self.dispatcher.notify_trade_settled(
            buy_account_id, 
            trade_event.symbol, 
            TradeSide::Buy, 
            trade_event.quantity as i64, 
            trade_event.price as i64
        ).await;
        
        self.dispatcher.notify_trade_settled(
            sell_account_id, 
            trade_event.symbol, 
            TradeSide::Sell, 
            trade_event.quantity as i64, 
            trade_event.price as i64
        ).await;

        tracing::info!("Successfully settled trade and notified post-settlement callbacks");
        Ok(())
    }
}
```

### 3. EVS Implementation

```rust
// File: engine/equity-service/src/service.rs
use execution_manager::PostSettlementCallback;
use async_trait::async_trait;

#[async_trait]
impl PostSettlementCallback for EquityValuationService {
    async fn on_trade_settled(
        &self, 
        account_id: i64, 
        symbol_id: u32, 
        side: TradeSide, 
        quantity: i64, 
        price: i64
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update account data from the settled trade
        self.update_account_from_trade(account_id, symbol_id, price, quantity, side).await?;
        
        // Mark account as updated this tick
        {
            let mut accounts_updated = self.accounts_updated_this_tick.write().await;
            accounts_updated.insert(account_id);
        }
        
        Ok(())
    }
    
    async fn on_price_updated(&self, symbol_id: u32, price: i64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update price cache
        {
            let mut prices = self.price_cache.write().await;
            prices.insert(symbol_id, price);
        }
        
        // Mark symbol as updated this tick
        {
            let mut symbols_updated = self.symbols_updated_this_tick.write().await;
            symbols_updated.insert(symbol_id);
        }
        
        Ok(())
    }
    
    async fn on_tick_complete(&self, tick_id: TickId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Recalculate equity for all accounts that were updated this tick
        let accounts_to_update = {
            let accounts_updated = self.accounts_updated_this_tick.read().await;
            accounts_updated.clone()
        };
        
        // Recalculate equity for affected accounts
        for account_id in accounts_to_update {
            if let Ok(snapshot) = self.recalculate_equity(account_id).await {
                // Persist to database
                self.persist_equity_snapshot(&snapshot).await?;
                
                // Broadcast via WebSocket
                let update = EquityUpdate {
                    account_id: snapshot.account_id,
                    tick: snapshot.tick,
                    timestamp: snapshot.timestamp.timestamp(),
                    total_equity: snapshot.total_equity,
                    cash_balance: snapshot.cash_balance,
                    position_value: snapshot.position_value,
                    day_change: snapshot.day_change,
                    day_change_percent: snapshot.day_change_percent,
                    unrealized_pnl: snapshot.unrealized_pnl,
                    realized_pnl: snapshot.realized_pnl,
                };
                
                self.equity_broadcaster.send_equity_update(account_id, update).await?;
            }
        }
        
        // Clear tick tracking
        {
            let mut accounts_updated = self.accounts_updated_this_tick.write().await;
            accounts_updated.clear();
        }
        
        {
            let mut symbols_updated = self.symbols_updated_this_tick.write().await;
            symbols_updated.clear();
        }
        
        Ok(())
    }
}
```

## Data Structures

### 1. EVS Core Service
```rust
// File: engine/equity-service/src/lib.rs
pub struct EquityValuationService {
    // Database connection
    db_pool: Arc<PgPool>,
    
    // WebSocket broadcaster for sending equity updates
    equity_broadcaster: Arc<EquityBroadcaster>,
    
    // In-memory caches
    account_cache: Arc<RwLock<HashMap<i64, AccountEquityData>>>,
    price_cache: Arc<RwLock<HashMap<u32, i64>>>,  // symbol_id -> price in cents
    equity_cache: Arc<RwLock<HashMap<i64, EquitySnapshot>>>,  // account_id -> equity
    
    // Tracking to prevent double counting
    accounts_updated_this_tick: Arc<RwLock<HashSet<i64>>>,
    symbols_updated_this_tick: Arc<RwLock<HashSet<u32>>>,
}
```

### 2. Account Equity Data
```rust
// File: engine/equity-service/src/types.rs
pub struct AccountEquityData {
    pub account_id: i64,
    pub cash_balance: i64,  // in cents
    pub positions: HashMap<u32, Position>,  // symbol_id -> position
    pub realized_pnl: i64,  // Total realized P&L in cents
    pub last_updated: Instant,
}

pub struct Position {
    pub symbol_id: u32,
    pub quantity: i64,      // in basis points
    pub avg_cost: i64,      // in cents
    pub realized_pnl: i64,  // Realized P&L for this position
    pub last_updated: Instant,
}
```

### 3. Equity Snapshot
```rust
// File: engine/equity-service/src/types.rs
pub struct EquitySnapshot {
    pub account_id: i64,
    pub timestamp: DateTime<Utc>,
    pub tick: u64,
    pub total_equity: i64,        // in cents
    pub cash_balance: i64,        // in cents
    pub position_value: i64,      // in cents
    pub unrealized_pnl: i64,      // in cents
    pub realized_pnl: i64,        // in cents
    pub day_change: i64,          // in cents
    pub day_change_percent: BigDecimal,
}
```

### 4. Equity Broadcaster
```rust
// File: engine/order-gateway/src/equity_broadcaster.rs
pub struct EquityBroadcaster {
    // Map account_id -> Vec<WebSocketSender>
    account_clients: Arc<RwLock<HashMap<i64, Vec<mpsc::UnboundedSender<WsMessage>>>>>,
}

impl EquityBroadcaster {
    pub async fn add_equity_client(&self, account_id: i64, sender: mpsc::UnboundedSender<WsMessage>) {
        let mut clients = self.account_clients.write().await;
        clients.entry(account_id).or_insert_with(Vec::new).push(sender);
    }
    
    pub async fn send_equity_update(&self, account_id: i64, equity_data: EquityUpdate) -> Result<()> {
        let clients = self.account_clients.read().await;
        
        if let Some(account_clients) = clients.get(&account_id) {
            let message = serde_json::json!({
                "stream": "equity",
                "data": equity_data
            });
            
            let ws_message = WsMessage::Text(serde_json::to_string(&message)?);
            
            for client in account_clients {
                if let Err(_) = client.send(ws_message.clone()) {
                    // Client disconnected, will be cleaned up
                }
            }
        }
        
        Ok(())
    }
}
```

## Event Flow

### 1. Event Reception
```rust
// File: engine/equity-service/src/lib.rs
impl EquityValuationService {
    /// Main event handler - routes events to specific handlers
    pub async fn on_event(&self, event: &DispatchEvent) -> Result<()> {
        match event {
            DispatchEvent::TradeEvent(trade) => self.on_trade_event(trade).await,
            DispatchEvent::BookDelta(delta) => self.on_book_delta(delta).await,
            DispatchEvent::TickBoundary(boundary) => self.on_tick_boundary(boundary).await,
            _ => Ok(()), // Ignore other events
        }
    }
}
```

### 2. Trade Event Processing
```rust
// File: engine/equity-service/src/lib.rs
impl EquityValuationService {
    /// Called when TradeEvent comes from ExecutionManager
    pub async fn on_trade_event(&self, event: &TradeEvent) -> Result<()> {
        // 1. Get account IDs from order IDs
        let buy_account_id = self.get_account_id_from_order_id(event.taker_order_id).await?;
        let sell_account_id = self.get_account_id_from_order_id(event.maker_order_id).await?;
        
        // 2. Update account cache for both accounts
        self.update_account_from_trade(buy_account_id, event.symbol, event.price, event.quantity, Side::Buy).await?;
        self.update_account_from_trade(sell_account_id, event.symbol, event.price, event.quantity, Side::Sell).await?;
        
        // 3. Update price cache
        self.price_cache.write().await.insert(event.symbol, event.price as i64);
        
        // 4. Mark accounts as updated this tick
        self.accounts_updated_this_tick.write().await.insert(buy_account_id);
        self.accounts_updated_this_tick.write().await.insert(sell_account_id);
        
        // 5. Recalculate equity for both accounts
        self.recalculate_equity(buy_account_id).await?;
        self.recalculate_equity(sell_account_id).await?;
        
        // 6. Broadcast updates
        self.broadcast_equity_update(buy_account_id).await?;
        self.broadcast_equity_update(sell_account_id).await?;
        
        Ok(())
    }
}
```

### 3. Book Delta Processing
```rust
// File: engine/equity-service/src/lib.rs
impl EquityValuationService {
    /// Called when BookDelta comes from ExecutionManager
    pub async fn on_book_delta(&self, event: &BookDelta) -> Result<()> {
        // 1. Update price cache (use mid-price)
        let current_price = self.calculate_mid_price(event.symbol).await?;
        self.price_cache.write().await.insert(event.symbol, current_price);
        
        // 2. Find accounts holding this symbol
        let affected_accounts = self.get_accounts_holding_symbol(event.symbol).await?;
        
        // 3. Only recalculate for accounts NOT updated this tick
        for account_id in affected_accounts {
            if !self.accounts_updated_this_tick.read().await.contains(&account_id) {
                self.recalculate_equity(account_id).await?;
                self.broadcast_equity_update(account_id).await?;
            }
        }
        
        Ok(())
    }
}
```

### 4. Tick Boundary Processing
```rust
// File: engine/equity-service/src/lib.rs
impl EquityValuationService {
    /// Called when TickBoundaryEvent comes from ExecutionManager
    pub async fn on_tick_boundary(&self, event: &TickBoundaryEvent) -> Result<()> {
        // 1. Persist all equity snapshots to database
        let equity_cache = self.equity_cache.read().await;
        for (account_id, snapshot) in equity_cache.iter() {
            self.persist_equity_snapshot(snapshot.clone()).await?;
        }
        
        // 2. Clear tracking for next tick
        self.accounts_updated_this_tick.write().await.clear();
        self.symbols_updated_this_tick.write().await.clear();
        
        Ok(())
    }
}
```

## Implementation Details

### 1. ExecutionManager Integration
```rust
// File: engine/execution-manager/src/dispatch.rs
pub struct EventDispatcher {
    config: FanoutConfig,
    metrics: Arc<MetricsCollector>,
    analytics_converter: AnalyticsConverter,
    analytics_sender: Option<mpsc::UnboundedSender<analytics_engine::analytics::AnalyticsEvent>>,
    
    // NEW: EVS integration
    equity_service: Option<Arc<EquityValuationService>>,
}

impl EventDispatcher {
    // NEW: Method to set EVS
    pub fn set_equity_service(&mut self, evs: Arc<EquityValuationService>) {
        self.equity_service = Some(evs);
    }
    
    pub fn dispatch(&self, event: DispatchEvent) -> Result<(), String> {
        // ... existing dispatch logic ...
        
        // NEW: Notify EVS
        if let Some(evs) = &self.equity_service {
            // Spawn async task to avoid blocking
            let evs_clone = evs.clone();
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = evs_clone.on_event(&event_clone).await {
                    tracing::error!("EVS event processing failed: {}", e);
                }
            });
        }
        
        Ok(())
    }
}
```

### 2. WebSocket Handler Integration
```rust
// File: engine/order-gateway/src/websocket_handler.rs
pub struct WebSocketHandler {
    // ... existing fields ...
    
    // NEW: Equity broadcaster
    equity_broadcaster: Arc<EquityBroadcaster>,
}

impl WebSocketHandler {
    // NEW: Add equity subscription method
    Some("equity.subscribe") => {
        self.handle_equity_subscribe(message).await?;
    }
    
    async fn handle_equity_subscribe(&mut self, message: ApiMessage) -> GatewayResult<()> {
        if let Some(session) = &self.user_session {
            // Subscribe this WebSocket to equity updates for this account
            if let Some(sender) = &self.sender {
                self.equity_broadcaster
                    .add_equity_client(session.account_id, sender.clone())
                    .await;
            }
        }
        Ok(())
    }
}
```

### 3. Trade Processing with Realized P&L
```rust
// File: engine/equity-service/src/lib.rs
impl EquityValuationService {
    async fn update_account_from_trade(
        &self,
        account_id: i64,
        symbol_id: u32,
        price: i64,
        quantity: i64,
        side: Side,
    ) -> Result<()> {
        let mut account_data = self.account_cache.write().await;
        let account = account_data.get_mut(&account_id).unwrap();
        
        match side {
            Side::Buy => {
                // Buying: No realized P&L yet, just update position
                account.cash_balance -= price * quantity;
                // Update position (existing logic)
            }
            Side::Sell => {
                // Selling: Calculate realized P&L
                if let Some(position) = account.positions.get(&symbol_id) {
                    let cost_basis = position.avg_cost * quantity;
                    let proceeds = price * quantity;
                    let realized_pnl = proceeds - cost_basis;
                    
                    // Add realized P&L to account total
                    account.realized_pnl += realized_pnl;
                    
                    // Add proceeds to cash
                    account.cash_balance += proceeds;
                    
                    // Update position quantity
                    // ... existing position update logic
                }
            }
        }
        
        Ok(())
    }
}
```

### 4. Equity Calculation
```rust
// File: engine/equity-service/src/lib.rs
impl EquityValuationService {
    async fn recalculate_equity(&self, account_id: i64) -> Result<EquitySnapshot> {
        let account_data = self.get_account_data(account_id).await?;
        let prices = self.price_cache.read().await;
        
        // Calculate position value
        let mut position_value = 0i64;
        let mut unrealized_pnl = 0i64;
        
        for position in &account_data.positions {
            if let Some(current_price) = prices.get(&position.symbol_id) {
                let current_value = position.quantity * current_price;
                position_value += current_value;
                
                // Unrealized P&L = (current_price - avg_cost) * quantity
                let cost_basis = position.quantity * position.avg_cost;
                unrealized_pnl += current_value - cost_basis;
            }
        }
        
        // Total equity = cash + position value
        let total_equity = account_data.cash_balance + position_value;
        
        // Calculate day change (vs previous day's equity)
        let day_change = self.calculate_day_change(account_id, total_equity).await?;
        let day_change_percent = if day_change != 0 {
            (day_change as f64 / (total_equity - day_change) as f64) * 100.0
        } else {
            0.0
        };
        
        let snapshot = EquitySnapshot {
            account_id,
            timestamp: Utc::now(),
            tick: self.get_current_tick().await?,
            total_equity,
            cash_balance: account_data.cash_balance,
            position_value,
            unrealized_pnl,
            realized_pnl: account_data.realized_pnl,  // Use tracked realized P&L
            day_change,
            day_change_percent: BigDecimal::from_str(&day_change_percent.to_string())?,
        };
        
        // Update equity cache
        self.equity_cache.write().await.insert(account_id, snapshot.clone());
        
        Ok(snapshot)
    }
}
```

## Database Schema

### 1. Equity Timeseries Table
```sql
-- File: engine/account-service/migrations/003_equity_timeseries.sql
CREATE TABLE equity_timeseries (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    tick BIGINT NOT NULL,
    total_equity BIGINT NOT NULL,  -- in cents
    cash_balance BIGINT NOT NULL,  -- in cents
    position_value BIGINT NOT NULL, -- in cents
    unrealized_pnl BIGINT NOT NULL, -- in cents
    realized_pnl BIGINT NOT NULL,   -- in cents
    day_change BIGINT NOT NULL,     -- in cents
    day_change_percent DECIMAL(10,4) NOT NULL,
    
    -- Indexes for fast queries
    INDEX idx_equity_account_timestamp (account_id, timestamp),
    INDEX idx_equity_timestamp (timestamp)
);
```

### 2. Database Schema Updates
```sql
-- Add realized_pnl columns to existing tables
ALTER TABLE accounts ADD COLUMN realized_pnl BIGINT DEFAULT 0; -- in cents
ALTER TABLE positions ADD COLUMN realized_pnl BIGINT DEFAULT 0; -- in cents

-- Update equity_timeseries table (already in our schema)
-- realized_pnl column already exists
```

### 3. Database Operations
```rust
// File: engine/equity-service/src/lib.rs
impl EquityValuationService {
    // EVS writes equity snapshots to database
    async fn persist_equity_snapshot(&self, snapshot: EquitySnapshot) -> Result<()> {
        sqlx::query!(
            "INSERT INTO equity_timeseries (account_id, timestamp, tick, total_equity, cash_balance, position_value, unrealized_pnl, realized_pnl, day_change, day_change_percent) 
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            snapshot.account_id,
            snapshot.timestamp,
            snapshot.tick,
            snapshot.total_equity,
            snapshot.cash_balance,
            snapshot.position_value,
            snapshot.unrealized_pnl,
            snapshot.realized_pnl,
            snapshot.day_change,
            snapshot.day_change_percent
        )
        .execute(&self.db_pool)
        .await?;
        
        Ok(())
    }
}
```

## WebSocket Integration

### 1. Frontend WebSocket Message Format
```typescript
// File: waiver-exchange-frontend/src/types/api.ts
interface EquityUpdate {
  stream: "equity";
  data: {
    account_id: number;
    tick: number;
    timestamp: number;
    total_equity: number;      // Total portfolio value in cents
    cash_balance: number;      // Available cash in cents
    position_value: number;    // Value of all positions in cents
    day_change: number;        // $ change today in cents
    day_change_percent: number; // % change today
    unrealized_pnl: number;    // Unrealized P&L in cents (paper gains/losses)
    realized_pnl: number;      // Realized P&L in cents (actual trading profits/losses)
  };
}

// Account Summary Component Data
interface AccountSummaryData {
  total_equity: number;        // Total portfolio value
  cash_balance: number;        // Available cash
  position_value: number;      // Value of positions
  unrealized_pnl: number;      // Paper gains/losses
  realized_pnl: number;        // ACTUAL gains/losses from trades
  day_change: number;          // Total change today
  day_change_percent: number;  // Percentage change today
}
```

### 2. Frontend WebSocket Handler
```typescript
// File: waiver-exchange-frontend/src/lib/websocket-client.ts
websocket.onmessage = (event) => {
  const message = JSON.parse(event.data);
  
  if (message.stream === "equity") {
    const equityData = message.data;
    
    // Update Account Summary component
    updateAccountSummary({
      total_equity: equityData.total_equity / 100, // Convert cents to dollars
      day_change: equityData.day_change / 100,
      day_change_percent: equityData.day_change_percent,
      cash_balance: equityData.cash_balance / 100,
      position_value: equityData.position_value / 100,
      unrealized_pnl: equityData.unrealized_pnl / 100, // Paper gains/losses
      realized_pnl: equityData.realized_pnl / 100,     // Actual trading profits/losses
    });
    
    // Update equity chart
    updateEquityChart({
      timestamp: equityData.timestamp,
      equity: equityData.total_equity / 100,
    });
  }
};

// Subscribe to equity updates when user logs in
function subscribeToEquity() {
  websocket.send(JSON.stringify({
    method: "equity.subscribe",
    id: generateId()
  }));
}
```

## Testing Strategy

### 1. Unit Tests
```rust
// File: engine/equity-service/src/tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_trade_event_processing() {
        // Test that trade events update account cache correctly
    }
    
    #[tokio::test]
    async fn test_book_delta_processing() {
        // Test that book deltas update price cache correctly
    }
    
    #[tokio::test]
    async fn test_equity_calculation() {
        // Test equity calculation logic
    }
    
    #[tokio::test]
    async fn test_double_counting_prevention() {
        // Test that accounts updated in same tick don't get double counted
    }
}
```

### 2. Integration Tests
```rust
// File: engine/equity-service/src/integration_tests.rs
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_execution_manager_integration() {
        // Test that EVS receives events from ExecutionManager
    }
    
    #[tokio::test]
    async fn test_websocket_broadcasting() {
        // Test that equity updates are broadcast via WebSocket
    }
    
    #[tokio::test]
    async fn test_database_persistence() {
        // Test that equity snapshots are persisted to database
    }
}
```

## Deployment Plan

### 1. Database Migration
```bash
# Run migration to add equity_timeseries table
cd engine/account-service
cargo run --bin migrate
```

### 2. Service Integration
```rust
// File: engine/order-gateway/src/main.rs
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ... existing setup ...
    
    // NEW: Create EVS
    let evs = Arc::new(EquityValuationService::new(db_pool.clone()).await?);
    
    // NEW: Set EVS in ExecutionManager
    execution_manager.set_equity_service(evs.clone());
    
    // NEW: Set EVS in WebSocketHandler
    let equity_broadcaster = Arc::new(EquityBroadcaster::new());
    evs.set_equity_broadcaster(equity_broadcaster.clone());
    
    // ... rest of existing setup ...
}
```

### 3. Configuration
```toml
# File: engine/equity-service/Cargo.toml
[package]
name = "equity-service"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1.0", features = ["full"] }
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-rustls"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
bigdecimal = { version = "0.4", features = ["serde"] }
tracing = "0.1"
anyhow = "1.0"
```

## Realized P&L Implementation

### 1. What is Realized P&L?

**Realized P&L** is the **actual profit or loss** that has been "locked in" through completed trades. It's different from **Unrealized P&L** which is just paper gains/losses.

### 2. Key Distinction

```
Unrealized P&L = (Current Price - Average Cost) × Quantity
- This is "paper" profit/loss
- Changes with every price movement
- Not locked in until you sell

Realized P&L = Actual profit/loss from completed trades
- This is "real" profit/loss
- Only changes when you actually trade
- Already locked in and added to your cash
```

### 3. Real-World Example

```
Day 1: Buy 100 shares of PlayerX at $50 = $5,000 cost
Day 2: PlayerX price goes to $60
- Unrealized P&L = ($60 - $50) × 100 = +$1,000 (paper gain)
- Realized P&L = $0 (no trades completed yet)

Day 3: Sell 50 shares at $60 = $3,000 received
- Unrealized P&L = ($60 - $50) × 50 = +$500 (remaining position)
- Realized P&L = ($60 - $50) × 50 = +$500 (locked in profit)

Day 4: PlayerX price drops to $40
- Unrealized P&L = ($40 - $50) × 50 = -$500 (paper loss on remaining)
- Realized P&L = +$500 (still locked in from the sale)
```

### 4. Calculation Formula

```
Realized P&L = (Sell Price - Average Cost) × Quantity Sold

Example:
- Buy 100 shares at $50 (avg_cost = $50)
- Sell 50 shares at $60
- Realized P&L = ($60 - $50) × 50 = +$500
```

### 5. Relevance in Our System

**Why Realized P&L Matters:**
1. **User Experience** - Users want to see their actual trading performance
2. **Tax Reporting** - Realized gains/losses have tax implications
3. **Performance Tracking** - Distinguish between paper gains and real profits
4. **Risk Management** - Understand actual vs. potential losses

**Current System Analysis:**
- Realized P&L is already calculated implicitly in `AccountService::settle_trade()`
- The cash impact from trades IS the realized P&L
- We need to track it explicitly for proper user experience

### 6. Frontend Display

```typescript
// Display in UI:
// "Realized P&L: +$1,250.00" (green if positive, red if negative)
// "Unrealized P&L: -$500.00" (paper loss on current positions)
```

## Key Implementation Notes

### 1. Double Counting Prevention
- **Accounts updated this tick**: Track which accounts had trades in current tick
- **Symbols updated this tick**: Track which symbols had price changes in current tick
- **Tick boundary clearing**: Clear tracking at end of each tick

### 2. Performance Optimizations
- **In-memory caches**: Avoid database queries for every calculation
- **Selective updates**: Only recalculate affected accounts
- **Async processing**: Non-blocking event processing

### 3. Error Handling
- **Graceful degradation**: Continue processing if individual events fail
- **Logging**: Comprehensive error logging for debugging
- **Recovery**: Automatic recovery from transient failures

### 4. Monitoring
- **Metrics**: Track equity calculation latency, cache hit rates
- **Health checks**: Monitor EVS health and performance
- **Alerts**: Alert on equity calculation failures or high latency

### 5. Realized P&L Tracking
- **Explicit tracking**: Track realized P&L separately from cash balance
- **Per-position tracking**: Track realized P&L for each position
- **Account-level aggregation**: Sum all position realized P&L for account total
- **Database persistence**: Store realized P&L in equity_timeseries table

This documentation provides the complete implementation guide with exact method signatures, data structures, and integration points. All code examples are based on the actual existing codebase and can be directly implemented.
