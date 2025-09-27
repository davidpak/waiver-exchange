//! Equity Valuation Service implementation
//!
//! Conversion Standards:
//! - Quantity: 10000 basis points = 1 share (matches database schema)
//! - Price: 1 cent = 1 unit
//! - Position Value: (quantity_bp * price_cents) / 10000

// Standard conversion factors
const QTY_SCALE: i64 = 10000;  // 10000 basis points = 1 share (matches database schema)
const CENTS: i64 = 1;          // 1 cent = 1 unit

use crate::config::EquityServiceConfig;
use crate::error::{EquityServiceError, Result};
use crate::types::{
    AccountEquityData, EquitySnapshot, Position,
};
use account_service::position::TradeSide;
use async_trait::async_trait;
use dashmap::DashMap;
use execution_manager::{DispatchEvent, TradeEvent, BookDelta, TickBoundaryEvent, PostSettlementCallback};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use chrono::Utc;
use bigdecimal::{BigDecimal, FromPrimitive};
use tokio::sync::RwLock;
use whistle::TickId;

/// Equity Valuation Service - calculates real-time equity for all accounts
pub struct EquityValuationService {
    config: EquityServiceConfig,
    db_pool: PgPool,
    
    // In-memory caches for performance
    account_cache: Arc<DashMap<i64, AccountEquityData>>,
    price_cache: Arc<RwLock<HashMap<u32, i64>>>, // symbol_id -> price in cents
    
    // Equity cache for snapshots
    equity_cache: Arc<RwLock<HashMap<i64, EquitySnapshot>>>,
    
    
    // Double counting prevention
    accounts_updated_this_tick: Arc<RwLock<HashSet<i64>>>,
    symbols_updated_this_tick: Arc<RwLock<HashSet<u32>>>,
}

impl EquityValuationService {
    /// Create a new Equity Valuation Service
    pub async fn new(config: EquityServiceConfig) -> Result<Self> {
        // Create database pool
        let db_pool = PgPool::connect(&config.database_url).await?;
        
        // Run migrations
        sqlx::migrate!("./migrations").run(&db_pool).await?;
        
        Ok(Self {
            config,
            db_pool,
            account_cache: Arc::new(DashMap::new()),
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            equity_cache: Arc::new(RwLock::new(HashMap::new())),
            accounts_updated_this_tick: Arc::new(RwLock::new(HashSet::new())),
            symbols_updated_this_tick: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    /// Process events from ExecutionManager
    pub async fn process_event(&self, event: DispatchEvent) -> Result<()> {
        match event {
            DispatchEvent::TradeEvent(_trade) => {
                // Skip trade events - we handle them via PostSettlementCallback instead
                // This prevents duplicate processing of the same trade
            }
            DispatchEvent::BookDelta(delta) => {
                self.process_book_delta(&delta).await?;
            }
            DispatchEvent::TickBoundary(boundary) => {
                self.process_tick_boundary(&boundary).await?;
            }
            _ => {
                // Other events don't affect equity directly
            }
        }
        Ok(())
    }


    /// Process a book delta event (price update)
    async fn process_book_delta(&self, delta: &BookDelta) -> Result<()> {
        // Update price cache
        {
            let mut prices = self.price_cache.write().await;
            prices.insert(delta.symbol, delta.price_level as i64);
        }
        
        // Mark symbol as updated this tick
        {
            let mut symbols_updated = self.symbols_updated_this_tick.write().await;
            symbols_updated.insert(delta.symbol);
        }
        
        Ok(())
    }

    /// Process a tick boundary event
    async fn process_tick_boundary(&self, _boundary: &TickBoundaryEvent) -> Result<()> {
        // Recalculate equity for all accounts that were updated this tick
        let accounts_to_update = {
            let accounts_updated = self.accounts_updated_this_tick.read().await;
            accounts_updated.clone()
        };
        
        let _symbols_to_update = {
            let symbols_updated = self.symbols_updated_this_tick.read().await;
            symbols_updated.clone()
        };
        
        // Recalculate equity for affected accounts
        for account_id in accounts_to_update {
            if let Ok(snapshot) = self.recalculate_equity(account_id).await {
                // Persist to database
                self.persist_equity_snapshot(&snapshot).await?;
                
                tracing::info!("🔥 EVS: Equity update persisted for account {}: ${}", account_id, snapshot.total_equity);
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

    /// Update account data from a trade
    async fn update_account_from_trade(
        &self,
        account_id: i64,
        symbol_id: u32,
        price: i64,
        quantity: i64,
        side: TradeSide,
    ) -> Result<()> {
        let mut account_data = self.get_or_create_account_data(account_id).await?;
        
        match side {
            TradeSide::Buy => {
                // Note: Cash balance, position quantity and average cost are updated by AccountService
                // EVS only handles realized P&L calculation
            }
            TradeSide::Sell => {
                // Selling: Calculate realized P&L using proper scaling
                if let Some(position) = account_data.positions.get(&symbol_id) {
                    // Use basis point scaling for all calculations
                    let cost_basis = (quantity * position.avg_cost) / QTY_SCALE;
                    let proceeds = (quantity * price) / QTY_SCALE;
                    let realized_pnl = proceeds - cost_basis;
                    
                    // Add realized P&L to account total
                    account_data.realized_pnl += realized_pnl;
                    
                    tracing::info!("🔥 EVS: Realized P&L calculation - Quantity: {} bp, Price: {} cents, Avg Cost: {} cents, Proceeds: {} cents, Cost Basis: {} cents, Realized P&L: {} cents, Total Realized: {} cents", 
                        quantity, price, position.avg_cost, proceeds, cost_basis, realized_pnl, account_data.realized_pnl);
                    
                    // Note: Cash balance and position quantity are updated by AccountService
                    // EVS only handles realized P&L calculation
                }
            }
        }
        
        // Update cache
        self.account_cache.insert(account_id, account_data);
        
        Ok(())
    }

    /// Recalculate equity for an account
    async fn recalculate_equity(&self, account_id: i64) -> Result<EquitySnapshot> {
        let account_data = self.get_account_data(account_id).await?;
        
        // Calculate position value using proper basis point scaling
        let mut position_value = 0i64;
        let mut unrealized_pnl = 0i64;
        
        for (symbol_id, position) in &account_data.positions {
            // Get current price - try cache first, then fetch from latest trade
            let current_price = self.get_current_price(*symbol_id).await?;
            
            // Use proper basis point scaling: (quantity_bp * price_cents) / QTY_SCALE
            let current_value = (position.quantity * current_price) / QTY_SCALE;
            position_value += current_value;
            
            // Unrealized P&L = (current_price - avg_cost) * quantity_bp / QTY_SCALE
            let cost_basis = (position.quantity * position.avg_cost) / QTY_SCALE;
            unrealized_pnl += current_value - cost_basis;
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
            timestamp: chrono::Utc::now(),
            tick: self.get_current_tick().await?,
            total_equity,
            cash_balance: account_data.cash_balance,
            position_value,
            unrealized_pnl,
            realized_pnl: account_data.realized_pnl,
            day_change,
            day_change_percent,
        };
        
        // Update equity cache
        self.equity_cache.write().await.insert(account_id, snapshot.clone());
        
        Ok(snapshot)
    }

    /// Recalculate equity for an account with a specific tick ID
    async fn recalculate_equity_with_tick(&self, account_id: i64, tick_id: TickId) -> Result<EquitySnapshot> {
        let account_data = self.get_account_data(account_id).await?;
        
        // Calculate position value using proper basis point scaling
        let mut position_value = 0i64;
        let mut unrealized_pnl = 0i64;
        
        for (symbol_id, position) in &account_data.positions {
            // Get current price - try cache first, then fetch from latest trade
            let current_price = self.get_current_price(*symbol_id).await?;
            
            tracing::info!("🔥 EVS: Position value calculation - Symbol: {}, Quantity: {} bp, Current Price: {} cents (${:.2}), Position Value: {} cents", 
                symbol_id, position.quantity, current_price, current_price as f64 / 100.0, (position.quantity * current_price) / QTY_SCALE);
            
            // Use proper basis point scaling: (quantity_bp * price_cents) / QTY_SCALE
            let current_value = (position.quantity * current_price) / QTY_SCALE;
            position_value += current_value;
            
            // Unrealized P&L = (current_price - avg_cost) * quantity_bp / QTY_SCALE
            let cost_basis = (position.quantity * position.avg_cost) / QTY_SCALE;
            unrealized_pnl += current_value - cost_basis;
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
            timestamp: chrono::Utc::now(),
            tick: tick_id,
            total_equity,
            cash_balance: account_data.cash_balance,
            position_value,
            unrealized_pnl,
            realized_pnl: account_data.realized_pnl,
            day_change,
            day_change_percent,
        };
        
        // Update equity cache
        self.equity_cache.write().await.insert(account_id, snapshot.clone());
        
        Ok(snapshot)
    }

    /// Get or create account data
    async fn get_or_create_account_data(&self, account_id: i64) -> Result<AccountEquityData> {
        if let Some(account_data) = self.account_cache.get(&account_id) {
            return Ok(account_data.clone());
        }
        
        // Load from database
        let account_data = self.load_account_data_from_db(account_id).await?;
        self.account_cache.insert(account_id, account_data.clone());
        
        Ok(account_data)
    }

    /// Get account data (from cache or database)
    async fn get_account_data(&self, account_id: i64) -> Result<AccountEquityData> {
        if let Some(account_data) = self.account_cache.get(&account_id) {
            return Ok(account_data.clone());
        }
        
        // Load from database
        let account_data = self.load_account_data_from_db(account_id).await?;
        self.account_cache.insert(account_id, account_data.clone());
        
        Ok(account_data)
    }

    /// Load account data from database
    async fn load_account_data_from_db(&self, account_id: i64) -> Result<AccountEquityData> {
        tracing::info!("🔍 EVS: Loading account data for account {}", account_id);
        
        // Load account balance
        let account = sqlx::query!(
            "SELECT currency_balance FROM accounts WHERE id = $1",
            account_id
        )
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or_else(|| EquityServiceError::AccountNotFound(account_id))?;
        
        tracing::info!("🔍 EVS: Account {} balance: {}", account_id, account.currency_balance.unwrap_or(0));
        
        // Load positions
        let positions = sqlx::query!(
            "SELECT symbol_id, quantity, avg_cost FROM positions WHERE account_id = $1",
            account_id
        )
        .fetch_all(&self.db_pool)
        .await?;
        
        tracing::info!("🔍 EVS: Found {} positions for account {}", positions.len(), account_id);
        
        let mut position_map = HashMap::new();
        for pos in positions {
            let position = Position {
                symbol_id: pos.symbol_id as u32,
                quantity: pos.quantity,
                avg_cost: pos.avg_cost,
                realized_pnl: 0,
                last_updated: Utc::now(),
            };
            position_map.insert(pos.symbol_id as u32, position);
        }
        
        // Load latest realized P&L from equity_timeseries
        let latest_equity = sqlx::query!(
            "SELECT realized_pnl FROM equity_timeseries WHERE account_id = $1 ORDER BY timestamp DESC LIMIT 1",
            account_id
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        let realized_pnl = latest_equity.map(|e| e.realized_pnl).unwrap_or(0);
        
        tracing::info!("🔍 EVS: Account {} loaded realized P&L: {} cents", account_id, realized_pnl);
        
        Ok(AccountEquityData {
            account_id,
            cash_balance: account.currency_balance.unwrap_or(0),
            positions: position_map,
            realized_pnl,
            last_updated: Utc::now(),
        })
    }

    /// Calculate day change for an account
    async fn calculate_day_change(&self, account_id: i64, current_equity: i64) -> Result<i64> {
        // Get the first equity snapshot of the day for this account
        let today = chrono::Utc::now().date_naive();
        
        let opening_equity = sqlx::query!(
            "SELECT total_equity FROM equity_timeseries 
             WHERE account_id = $1 AND DATE(timestamp) = $2 
             ORDER BY timestamp ASC LIMIT 1",
            account_id,
            today
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        match opening_equity {
            Some(record) => Ok(current_equity - record.total_equity),
            None => {
                // No opening equity found, assume this is the first snapshot of the day
                Ok(0)
            }
        }
    }

    /// Get current price for a symbol
    async fn get_current_price(&self, symbol_id: u32) -> Result<i64> {
        // Try cache first
        {
            let prices = self.price_cache.read().await;
            if let Some(price) = prices.get(&symbol_id) {
                return Ok(*price);
            }
        }
        
        // Cache miss - fetch from latest trade
        let latest_trade = sqlx::query!(
            "SELECT price FROM trades WHERE symbol_id = $1 ORDER BY timestamp DESC LIMIT 1",
            symbol_id as i32
        )
        .fetch_optional(&self.db_pool)
        .await?;
        
        match latest_trade {
            Some(trade) => {
                let price = trade.price as i64;
                // Update cache
                {
                    let mut prices = self.price_cache.write().await;
                    prices.insert(symbol_id, price);
                }
                Ok(price)
            }
            None => {
                // No trades found, use a default price or return error
                tracing::warn!("No price found for symbol {}", symbol_id);
                Ok(1000) // Default price of $10.00
            }
        }
    }

    /// Get current tick (placeholder implementation)
    async fn get_current_tick(&self) -> Result<TickId> {
        // In a real implementation, this would get the current tick from the system
        // For now, we'll use a simple counter
        Ok(1)
    }

    /// Persist equity snapshot to database
    async fn persist_equity_snapshot(&self, snapshot: &EquitySnapshot) -> Result<()> {
        tracing::info!("💾 EVS: Persisting equity snapshot for account {}: ${}", 
            snapshot.account_id, snapshot.total_equity);
        
        sqlx::query!(
            r#"
            INSERT INTO equity_timeseries 
            (account_id, timestamp, tick, total_equity, cash_balance, position_value, 
             unrealized_pnl, realized_pnl, day_change, day_change_percent)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            snapshot.account_id,
            snapshot.timestamp,
            snapshot.tick as i64,
            snapshot.total_equity,
            snapshot.cash_balance,
            snapshot.position_value,
            snapshot.unrealized_pnl,
            snapshot.realized_pnl,
            snapshot.day_change,
            BigDecimal::from_f64(snapshot.day_change_percent).unwrap_or(BigDecimal::from(0))
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| {
            tracing::error!("💾 EVS: Failed to persist equity snapshot: {}", e);
            EquityServiceError::Database(e)
        })?;
        
        tracing::info!("💾 EVS: Successfully persisted equity snapshot for account {}", snapshot.account_id);
        Ok(())
    }




    /// Get current equity for an account
    pub async fn get_current_equity(&self, account_id: i64) -> Result<EquitySnapshot> {
        if let Some(snapshot) = self.equity_cache.read().await.get(&account_id) {
            return Ok(snapshot.clone());
        }
        
        // Recalculate if not in cache
        self.recalculate_equity(account_id).await
    }
}

#[async_trait]
impl PostSettlementCallback for EquityValuationService {
    async fn on_trade_settled(
        &self, 
        account_id: i64, 
        symbol_id: u32, 
        side: TradeSide, 
        quantity: i64, 
        price: i64
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Convert quantity from real shares to basis points
        let quantity_basis_points = quantity * QTY_SCALE;
        
        tracing::info!("🔥 EVS: Processing trade settlement - Account: {}, Symbol: {}, Side: {:?}, Quantity: {} shares ({} bp), Price: {} cents (${:.2})", 
            account_id, symbol_id, side, quantity, quantity_basis_points, price, price as f64 / 100.0);
        
        // Update account data from the settled trade
        self.update_account_from_trade(account_id, symbol_id, price, quantity_basis_points, side).await?;
        
        // Mark account as updated this tick
        {
            let mut accounts_updated = self.accounts_updated_this_tick.write().await;
            accounts_updated.insert(account_id);
            tracing::info!("🔥 EVS: Marked account {} as updated this tick. Total accounts updated: {}", 
                account_id, accounts_updated.len());
        }
        
        tracing::info!("🔥 EVS: Successfully processed trade settlement for account {}", account_id);
        Ok(())
    }
    
    async fn on_price_updated(&self, symbol_id: u32, price: i64) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("🔥 EVS: Received price update callback - Symbol: {}, Price: {}", symbol_id, price);
        
        // Update price cache
        {
            let mut prices = self.price_cache.write().await;
            prices.insert(symbol_id, price);
        }
        
        // Mark symbol as updated this tick
        {
            let mut symbols_updated = self.symbols_updated_this_tick.write().await;
            symbols_updated.insert(symbol_id);
            tracing::info!("🔥 EVS: Marked symbol {} as updated this tick. Total symbols updated: {}", 
                symbol_id, symbols_updated.len());
        }
        
        tracing::info!("🔥 EVS: Successfully processed price update for symbol {}", symbol_id);
        Ok(())
    }
    
    async fn on_tick_complete(&self, tick_id: TickId) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("🔥 EVS: Received tick complete callback - Tick: {}", tick_id);
        
        // Recalculate equity for all accounts that were updated this tick
        let accounts_to_update = {
            let accounts_updated = self.accounts_updated_this_tick.read().await;
            let accounts = accounts_updated.clone();
            tracing::info!("🔥 EVS: Found {} accounts to update for tick {}", accounts.len(), tick_id);
            accounts
        };
        
        // Recalculate equity for affected accounts
        for account_id in accounts_to_update {
            tracing::info!("🔥 EVS: Recalculating equity for account {} at tick {}", account_id, tick_id);
            if let Ok(snapshot) = self.recalculate_equity_with_tick(account_id, tick_id).await {
                tracing::info!("🔥 EVS: Equity calculated - Account: {}, Total: {}, Cash: {}, Position: {}", 
                    account_id, snapshot.total_equity, snapshot.cash_balance, snapshot.position_value);
                
                // Persist to database
                self.persist_equity_snapshot(&snapshot).await?;
                
                tracing::info!("🔥 EVS: Equity update persisted for account {}: ${}", account_id, snapshot.total_equity);
            } else {
                tracing::error!("🔥 EVS: Failed to recalculate equity for account {}", account_id);
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
        
        tracing::info!("🔥 EVS: Completed tick {} processing", tick_id);
        Ok(())
    }

}
