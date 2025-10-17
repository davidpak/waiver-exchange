//! REST API endpoints for the OrderGateway
//!
//! This module provides REST API endpoints for symbol information, price history,
//! account data, and order placement.

use crate::cache::CacheManager;
use chrono::Timelike;
use num_traits::cast::ToPrimitive;
use num_traits::FromPrimitive;
use persistence::snapshot::SnapshotManager;
use player_registry::PlayerRegistry;
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use sqlx::PgPool;
use std::sync::Arc;
use warp::Filter;

/// Custom error for not found responses
#[derive(Debug)]
struct NotFoundError(ErrorResponse);

impl warp::reject::Reject for NotFoundError {}

/// Symbol information response
#[derive(Serialize, Deserialize)]
pub struct SymbolInfoResponse {
    pub symbol_id: u32,
    pub name: String,
    pub position: String,
    pub team: String,
    pub projected_points: f64,
    pub last_updated: String,
}

/// Price history response
#[derive(Serialize, Deserialize)]
pub struct PriceHistoryResponse {
    pub symbol_id: String,
    pub period: String,
    pub interval: String,
    pub candles: Vec<CandleData>,
}

/// Candle data for price history
#[derive(Serialize, Deserialize)]
pub struct CandleData {
    pub timestamp: String,
    pub open: u64,
    pub high: u64,
    pub low: u64,
    pub close: u64,
    pub volume: u64,
}

/// Account summary response
#[derive(Serialize, Deserialize)]
pub struct AccountSummaryResponse {
    pub account_id: i64,
    pub balance: u64,
    pub total_equity: u64,
    pub position_value: u64,
    pub day_change: i64,
    pub day_change_percent: f64,
    pub buying_power: u64,
    pub unrealized_pnl: i64,
    pub realized_pnl: i64,
    pub last_updated: String,
}

/// Equity history request parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct EquityHistoryParams {
    pub account_id: i64,
    pub start_date: Option<String>, // YYYY-MM-DD format
    pub end_date: Option<String>,   // YYYY-MM-DD format
}

/// Equity history response
#[derive(Debug, Serialize, Deserialize)]
pub struct EquityHistoryResponse {
    pub account_id: i64,
    pub snapshots: Vec<EquitySnapshot>,
    pub total_days: usize,
}

/// Individual equity snapshot
#[derive(Debug, Serialize, Deserialize)]
pub struct EquitySnapshot {
    pub date: String,            // YYYY-MM-DD
    pub total_equity: u64,       // In cents
    pub cash_balance: u64,       // In cents
    pub position_value: u64,     // In cents
    pub unrealized_pnl: i64,     // In cents
    pub realized_pnl: i64,       // In cents
    pub day_change: i64,         // In cents
    pub day_change_percent: f64, // Percentage
}

/// Price history query parameters
#[derive(Deserialize)]
pub struct PriceHistoryParams {
    pub period: String,
    pub interval: String,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
    pub timestamp: String,
}

/// Error detail
#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

/// Snapshot response structure
#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotResponse {
    pub id: String,
    pub timestamp: String,
    pub tick: u64,
    pub state: SnapshotState,
}

/// Snapshot state structure
#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotState {
    pub order_books: std::collections::HashMap<String, OrderBookData>,
    pub active_symbols: Vec<u32>,
    pub config: SystemConfig,
    pub stats: SystemStats,
}

/// Order book data structure
#[derive(Debug, Serialize, Deserialize)]
pub struct OrderBookData {
    pub symbol_id: u32,
    pub buy_orders: std::collections::HashMap<String, u64>, // price in cents -> quantity
    pub sell_orders: std::collections::HashMap<String, u64>, // price in cents -> quantity
    pub last_trade_price: Option<u64>,                      // price in cents
    pub last_trade_quantity: Option<u64>,                   // number of shares
    pub last_trade_timestamp: Option<String>,               // ISO 8601 format
}

/// System configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemConfig {
    pub max_symbols: u32,
    pub max_accounts: u32,
    pub tick_duration_ns: u64,
}

/// System statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemStats {
    pub total_orders: u64,
    pub total_trades: u64,
    pub total_volume: u64,
    pub current_tick: u64,
    pub uptime_seconds: u64,
}

/// Get symbol information by symbol ID
pub async fn get_symbol_info(
    symbol_id: u32,
    registry: Arc<PlayerRegistry>,
    cache: Arc<CacheManager>,
) -> Result<impl warp::Reply, warp::Rejection> {
    // Try to get from cache first
    if let Ok(Some(cached_response)) =
        cache.get_cached_symbol_info::<SymbolInfoResponse>(symbol_id).await
    {
        tracing::debug!("Symbol info cache hit for symbol {}", symbol_id);
        return Ok(warp::reply::json(&cached_response));
    }

    // Cache miss - get from registry
    match registry.get_by_symbol_id(symbol_id) {
        Ok(player) => {
            let response = SymbolInfoResponse {
                symbol_id: symbol_id,
                name: player.name.clone(),
                position: player.position.clone(),
                team: player.team.clone(),
                projected_points: player.projected_points,
                last_updated: chrono::Utc::now().to_rfc3339(),
            };

            // Cache the response
            if let Err(e) = cache.cache_symbol_info(symbol_id, &response).await {
                tracing::warn!("Failed to cache symbol info for {}: {}", symbol_id, e);
            }

            Ok(warp::reply::json(&response))
        }
        Err(_) => {
            let error = ErrorResponse {
                error: ErrorDetail {
                    code: "SYMBOL_NOT_FOUND".to_string(),
                    message: format!("Symbol with ID '{}' not found", symbol_id),
                    details: Some(serde_json::json!({
                        "symbol_id": symbol_id
                    })),
                },
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
            Err(warp::reject::custom(NotFoundError(error)))
        }
    }
}

/// Get all players for search functionality
pub async fn get_all_players(
    registry: Arc<PlayerRegistry>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let players: Vec<SymbolInfoResponse> = registry
        .get_all_symbols()
        .iter()
        .map(|symbol| SymbolInfoResponse {
            symbol_id: symbol.symbol_id,
            name: symbol.name.clone(),
            position: symbol.position.clone(),
            team: symbol.team.clone(),
            projected_points: symbol.projected_points,
            last_updated: chrono::Utc::now().to_rfc3339(),
        })
        .collect();

    Ok(warp::reply::json(&players))
}

/// Current price response
#[derive(Serialize, Deserialize)]
pub struct CurrentPriceResponse {
    pub symbol_id: u32,
    pub price: u64,
    pub source: String,
    pub last_updated: String,
}

/// Bulk prices response
#[derive(Serialize, Deserialize)]
pub struct BulkPricesResponse {
    pub prices: std::collections::HashMap<String, u64>, // symbol_id -> price in cents
    pub last_updated: String,
}

/// Get current price for a symbol (checks multiple sources)
pub async fn get_current_price(
    symbol_id: u32,
    db_pool: Arc<PgPool>,
) -> Result<impl warp::Reply, warp::Rejection> {
    // Try to get price from multiple sources in order of preference
    
    // 1. Try to get from fair prices (RPE engine)
    if let Ok(Some(fair_price)) = get_fair_price_for_symbol(symbol_id, &db_pool).await {
        let response = CurrentPriceResponse {
            symbol_id,
            price: fair_price.fair_cents as u64,
            source: "fair_price".to_string(),
            last_updated: fair_price.updated_at.to_rfc3339(),
        };
        return Ok(warp::reply::json(&response));
    }
    
    // 2. Try to get from price history
    if let Ok(Some(price)) = get_latest_price_from_history(symbol_id, &db_pool).await {
        let response = CurrentPriceResponse {
            symbol_id,
            price: price as u64,
            source: "price_history".to_string(),
            last_updated: chrono::Utc::now().to_rfc3339(),
        };
        return Ok(warp::reply::json(&response));
    }
    
    // 3. Fallback to default price
    let response = CurrentPriceResponse {
        symbol_id,
        price: 1000, // $10.00 default
        source: "default".to_string(),
        last_updated: chrono::Utc::now().to_rfc3339(),
    };
    
    Ok(warp::reply::json(&response))
}

/// Get fair price for a symbol from RPE engine
async fn get_fair_price_for_symbol(symbol_id: u32, db_pool: &PgPool) -> Result<Option<FairPriceData>, sqlx::Error> {
    // Directly use symbol_id as player_id since we now store symbol_id in the player_id column
    // Get the most recent record regardless of source
    let fair_price_row = sqlx::query!(
        "SELECT fair_cents, source, confidence_score, ts FROM rpe_fair_prices WHERE player_id = $1 ORDER BY ts DESC LIMIT 1",
        symbol_id as i32
    )
    .fetch_optional(db_pool)
    .await?;
    
    // Also get all records for debugging
    let all_records = sqlx::query!(
        "SELECT fair_cents, source, confidence_score, ts FROM rpe_fair_prices WHERE player_id = $1 ORDER BY ts DESC",
        symbol_id as i32
    )
    .fetch_all(db_pool)
    .await?;
    
    tracing::info!("All records for symbol {}: {:?}", symbol_id, all_records);
    
    match fair_price_row {
        Some(row) => {
            // Log for debugging
            tracing::info!("Found fair price for symbol {}: {} cents, source: {:?}, ts: {:?}", 
                symbol_id, row.fair_cents, row.source, row.ts);
            
            Ok(Some(FairPriceData {
                fair_cents: row.fair_cents,
                source: row.source.unwrap_or_else(|| "unknown".to_string()),
                confidence_score: row.confidence_score.unwrap_or_default().to_f64().unwrap_or(0.0),
                updated_at: row.ts,
            }))
        },
        None => {
            tracing::warn!("No fair price found for symbol {}", symbol_id);
            Ok(None)
        }
    }
}

/// Get latest price from price history
async fn get_latest_price_from_history(symbol_id: u32, db_pool: &PgPool) -> Result<Option<i64>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT close_price FROM price_history WHERE symbol_id = $1 ORDER BY timestamp DESC LIMIT 1",
        symbol_id as i32
    )
    .fetch_optional(db_pool)
    .await?;
    
    Ok(row.map(|r| r.close_price))
}

/// Get bulk prices for all symbols
pub async fn get_bulk_prices(
    db_pool: Arc<PgPool>,
) -> Result<impl warp::Reply, warp::Rejection> {
    // Get all fair prices from RPE engine
    let fair_prices = sqlx::query!(
        "SELECT DISTINCT ON (player_id) player_id, fair_cents, ts FROM rpe_fair_prices ORDER BY player_id, ts DESC"
    )
    .fetch_all(&*db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch bulk prices: {}", e);
        warp::reject::custom(NotFoundError(ErrorResponse {
            error: ErrorDetail {
                code: "DATABASE_ERROR".to_string(),
                message: "Failed to fetch bulk prices".to_string(),
                details: Some(serde_json::Value::String(e.to_string())),
            },
            timestamp: chrono::Utc::now().to_rfc3339(),
        }))
    })?;

    // Convert to HashMap
    let mut prices = std::collections::HashMap::new();
    for row in fair_prices {
        prices.insert(row.player_id.to_string(), row.fair_cents as u64);
    }

    let response = BulkPricesResponse {
        prices,
        last_updated: chrono::Utc::now().to_rfc3339(),
    };

    Ok(warp::reply::json(&response))
}

/// Fair price data structure
#[derive(Debug)]
struct FairPriceData {
    fair_cents: i64,
    source: String,
    confidence_score: f64,
    updated_at: chrono::DateTime<chrono::Utc>,
}

/// Get price history for a symbol
pub async fn get_price_history(
    symbol_id: u32,
    params: PriceHistoryParams,
    db_pool: Arc<PgPool>,
) -> Result<impl warp::Reply, warp::Rejection> {
    // Query price history from database
    let rows = sqlx::query!(
        "SELECT timestamp, open_price, high_price, low_price, close_price, volume 
         FROM price_history 
         WHERE symbol_id = $1 
         ORDER BY timestamp DESC 
         LIMIT 100",
        symbol_id as i32
    )
    .fetch_all(&*db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to query price history: {}", e);
        warp::reject::custom(NotFoundError(ErrorResponse {
            error: ErrorDetail {
                code: "DATABASE_ERROR".to_string(),
                message: "Failed to retrieve price history".to_string(),
                details: Some(serde_json::Value::String(e.to_string())),
            },
            timestamp: chrono::Utc::now().to_rfc3339(),
        }))
    })?;

    // Convert database rows to candle data
    let candles: Vec<CandleData> = rows
        .into_iter()
        .map(|row| CandleData {
            timestamp: row.timestamp.format("%Y-%m-%dT%H:%M:%S%.fZ").to_string(),
            open: row.open_price as u64,
            high: row.high_price as u64,
            low: row.low_price as u64,
            close: row.close_price as u64,
            volume: row.volume as u64,
        })
        .collect();

    let response = PriceHistoryResponse {
        symbol_id: symbol_id.to_string(),
        period: params.period,
        interval: params.interval,
        candles,
    };

    Ok(warp::reply::json(&response))
}

/// Get account summary
pub async fn get_account_summary(
    account_id: i64,
    db_pool: Arc<PgPool>,
) -> Result<impl warp::Reply, warp::Rejection> {
    // Query the accounts table
    match sqlx::query!(
        "SELECT id, currency_balance, last_updated FROM accounts WHERE id = $1",
        account_id
    )
    .fetch_one(&*db_pool)
    .await
    {
        Ok(row) => {
            let balance = row.currency_balance.unwrap_or(0) as u64;

            // Try to get the latest equity data from equity_timeseries
            let equity_data = sqlx::query!(
                "SELECT total_equity, cash_balance, position_value, unrealized_pnl, realized_pnl, day_change, day_change_percent, created_at
                 FROM equity_timeseries 
                 WHERE account_id = $1 
                 ORDER BY created_at DESC 
                 LIMIT 1",
                account_id
            )
            .fetch_optional(&*db_pool)
            .await
            .unwrap_or(None);

            let (total_equity, position_value, day_change, day_change_percent, unrealized_pnl, realized_pnl, last_updated) = if let Some(equity) = equity_data {
                (
                    equity.total_equity as u64,
                    equity.position_value as u64,
                    equity.day_change,
                    equity.day_change_percent.to_f64().unwrap_or(0.0),
                    equity.unrealized_pnl,
                    equity.realized_pnl,
                    match equity.created_at {
                        Some(dt) => dt.format("%Y-%m-%dT%H:%M:%S%.fZ").to_string(),
                        None => chrono::Utc::now().to_rfc3339(),
                    }
                )
            } else {
                // Fallback to calculating on-demand if no equity_timeseries data
                let calculated_equity = calculate_total_equity(&db_pool, account_id, balance).await;
                (
                    calculated_equity,
                    0u64, // position_value
                    0i64,
                    0.0,
                    0i64, // unrealized_pnl
                    0i64, // realized_pnl
                    row.last_updated
                        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.fZ").to_string())
                        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339())
                )
            };

            let response = AccountSummaryResponse {
                account_id: row.id,
                balance,
                total_equity,
                position_value,
                day_change,
                day_change_percent,
                buying_power: balance, // For now, buying power = balance (no margin)
                unrealized_pnl,
                realized_pnl,
                last_updated,
            };
            Ok(warp::reply::json(&response))
        }
        Err(_) => {
            // Return mock data if account doesn't exist
            let response = AccountSummaryResponse {
                account_id,
                balance: 100000, // $1000 in cents
                total_equity: 100000,
                position_value: 0,
                day_change: 0,
                day_change_percent: 0.0,
                buying_power: 100000,
                unrealized_pnl: 0,
                realized_pnl: 0,
                last_updated: chrono::Utc::now().to_rfc3339(),
            };
            Ok(warp::reply::json(&response))
        }
    }
}

/// Calculate total equity: cash balance + position values
async fn calculate_total_equity(db_pool: &PgPool, account_id: i64, cash_balance: u64) -> u64 {
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
                    let current_price = get_current_price_legacy(db_pool, position.symbol_id as i32).await;
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

/// Get current price for a symbol (from price_history table) - legacy function
async fn get_current_price_legacy(db_pool: &PgPool, symbol_id: i32) -> u64 {
    match sqlx::query!(
        "SELECT close_price FROM price_history WHERE symbol_id = $1 ORDER BY timestamp DESC LIMIT 1",
        symbol_id
    )
    .fetch_one(db_pool)
    .await
    {
        Ok(row) => row.close_price as u64,
        Err(_) => {
            // If no price history, use projected_points as fallback
            // This is a temporary fallback until we have real price data
            0 // Will be replaced with projected_points lookup
        }
    }
}

/// Get equity history for an account
pub async fn get_equity_history(
    params: EquityHistoryParams,
    db_pool: Arc<PgPool>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let start_date = params
        .start_date
        .unwrap_or_else(|| {
            (chrono::Utc::now() - chrono::Duration::days(365)).format("%Y-%m-%d").to_string()
        })
        .parse::<chrono::NaiveDate>()
        .unwrap_or_else(|_| chrono::Utc::now().date_naive() - chrono::Duration::days(365));
    let end_date = params
        .end_date
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string())
        .parse::<chrono::NaiveDate>()
        .unwrap_or_else(|_| chrono::Utc::now().date_naive());

    let rows = sqlx::query!(
        "SELECT created_at, total_equity, cash_balance, position_value, unrealized_pnl, realized_pnl, day_change, day_change_percent
         FROM equity_timeseries 
         WHERE account_id = $1 AND DATE(created_at) BETWEEN $2 AND $3
         ORDER BY created_at ASC",
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

    let total_days = rows.len();
    let snapshots: Vec<EquitySnapshot> = rows
        .into_iter()
        .map(|row| EquitySnapshot {
            date: match row.created_at {
                Some(dt) => dt.format("%Y-%m-%d").to_string(),
                None => chrono::Utc::now().format("%Y-%m-%d").to_string(),
            },
            total_equity: row.total_equity as u64,
            cash_balance: row.cash_balance as u64,
            position_value: row.position_value as u64,
            unrealized_pnl: row.unrealized_pnl,
            realized_pnl: row.realized_pnl,
            day_change: row.day_change,
            day_change_percent: row.day_change_percent.to_f64().unwrap_or(0.0),
        })
        .collect();

    let response = EquityHistoryResponse { account_id: params.account_id, snapshots, total_days };

    Ok(warp::reply::json(&response))
}

/// Create daily equity snapshots (admin/test endpoint)
pub async fn create_daily_snapshots(
    db_pool: Arc<PgPool>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let today = chrono::Utc::now().date_naive();

    // Get all accounts
    let accounts = match sqlx::query!("SELECT id FROM accounts").fetch_all(&*db_pool).await {
        Ok(accounts) => accounts,
        Err(e) => {
            tracing::error!("Failed to get accounts: {}", e);
            let error = ErrorResponse {
                error: ErrorDetail {
                    code: "DATABASE_ERROR".to_string(),
                    message: "Failed to get accounts".to_string(),
                    details: Some(serde_json::Value::String(e.to_string())),
                },
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
            return Err(warp::reject::custom(NotFoundError(error)));
        }
    };

    let mut snapshots_created = 0;

    for account in &accounts {
        let account_id = account.id;

        // Get account balance
        let balance =
            match sqlx::query!("SELECT currency_balance FROM accounts WHERE id = $1", account_id)
                .fetch_optional(&*db_pool)
                .await
            {
                Ok(Some(row)) => row.currency_balance.unwrap_or(0),
                Ok(None) => {
                    tracing::warn!("Account {} not found", account_id);
                    continue;
                }
                Err(e) => {
                    tracing::error!("Failed to get balance for account {}: {}", account_id, e);
                    continue;
                }
            };

        // Calculate total equity using existing logic
        let total_equity = calculate_total_equity(&db_pool, account_id, balance as u64).await;
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
            today - chrono::Duration::days(1)
        )
        .fetch_optional(&*db_pool)
        .await
        .unwrap_or(None);

        let (day_change, day_change_percent) = if let Some(prev) = previous_snapshot {
            let change = total_equity as i64 - prev.total_equity;
            let percent = if prev.total_equity > 0 {
                (change as f64 / prev.total_equity as f64) * 100.0
            } else {
                0.0
            };
            (change, BigDecimal::from_f64(percent).unwrap_or(BigDecimal::new(0.into(), 0)))
        } else {
            (0, BigDecimal::new(0.into(), 0)) // First day, no change
        };

        // Insert snapshot
        match sqlx::query!(
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
        .execute(&*db_pool)
        .await
        {
            Ok(_) => {
                snapshots_created += 1;
                tracing::info!("Created snapshot for account {}: equity={}, balance={}, position_value={}", 
                    account_id, total_equity, balance, position_value);
            }
            Err(e) => {
                tracing::error!("Failed to create snapshot for account {}: {}", account_id, e);
            }
        }
    }

    let response = serde_json::json!({
        "status": "success",
        "message": format!("Created {} daily equity snapshots", snapshots_created),
        "snapshots_created": snapshots_created,
        "total_accounts": accounts.len(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    Ok(warp::reply::json(&response))
}

/// Test scheduler logic (admin/test endpoint)
pub async fn test_scheduler_logic(
    db_pool: Arc<PgPool>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let now = chrono::Utc::now();

    let response = serde_json::json!({
        "status": "success",
        "message": "Scheduler status endpoint",
        "current_time": now.to_rfc3339(),
        "current_hour": now.hour(),
        "current_minute": now.minute(),
        "scheduler_runs_at": "21:00 UTC (4 PM EST)",
        "next_run_in_hours": if now.hour() < 21 { 21 - now.hour() } else { 24 - now.hour() + 21 },
        "scheduler_status": "Running in background",
        "timestamp": now.to_rfc3339()
    });

    Ok(warp::reply::json(&response))
}

/// Get current snapshot (live order book data)
pub async fn get_current_snapshot(
    snapshot_manager: Arc<SnapshotManager>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match snapshot_manager.load_latest_snapshot().await {
        Ok(Some(snapshot)) => {
            // Convert snapshot to our response format
            let mut order_books = std::collections::HashMap::new();

            for (symbol_id, order_book) in &snapshot.state.order_books {
                let mut buy_orders = std::collections::HashMap::new();
                let mut sell_orders = std::collections::HashMap::new();

                // Convert buy orders (price -> quantity)
                for (price, quantity) in &order_book.buy_orders {
                    buy_orders.insert(price.to_string(), *quantity);
                }

                // Convert sell orders (price -> quantity)
                for (price, quantity) in &order_book.sell_orders {
                    sell_orders.insert(price.to_string(), *quantity);
                }

                let order_book_data = OrderBookData {
                    symbol_id: *symbol_id,
                    buy_orders,
                    sell_orders,
                    last_trade_price: order_book.last_trade_price,
                    last_trade_quantity: order_book.last_trade_quantity,
                    last_trade_timestamp: order_book.last_trade_timestamp.map(|dt| dt.to_rfc3339()),
                };

                order_books.insert(symbol_id.to_string(), order_book_data);
            }

            let response = SnapshotResponse {
                id: snapshot.id.to_string(),
                timestamp: snapshot.timestamp.to_rfc3339(),
                tick: snapshot.tick,
                state: SnapshotState {
                    order_books,
                    active_symbols: snapshot.state.active_symbols.clone(),
                    config: SystemConfig {
                        max_symbols: snapshot.state.config.max_symbols,
                        max_accounts: snapshot.state.config.max_accounts,
                        tick_duration_ns: snapshot.state.config.tick_duration_ns,
                    },
                    stats: SystemStats {
                        total_orders: snapshot.state.stats.total_orders,
                        total_trades: snapshot.state.stats.total_trades,
                        total_volume: snapshot.state.stats.total_volume,
                        current_tick: snapshot.state.stats.current_tick,
                        uptime_seconds: snapshot.state.stats.uptime_seconds,
                    },
                },
            };

            Ok(warp::reply::json(&response))
        }
        Ok(None) => {
            // No snapshots available, return empty state
            let response = SnapshotResponse {
                id: uuid::Uuid::new_v4().to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                tick: 0,
                state: SnapshotState {
                    order_books: std::collections::HashMap::new(),
                    active_symbols: vec![],
                    config: SystemConfig {
                        max_symbols: 100,
                        max_accounts: 1000,
                        tick_duration_ns: 1000000,
                    },
                    stats: SystemStats {
                        total_orders: 0,
                        total_trades: 0,
                        total_volume: 0,
                        current_tick: 0,
                        uptime_seconds: 0,
                    },
                },
            };
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            tracing::error!("Failed to load snapshot: {}", e);
            let error = ErrorResponse {
                error: ErrorDetail {
                    code: "SNAPSHOT_ERROR".to_string(),
                    message: "Failed to load current snapshot".to_string(),
                    details: Some(serde_json::Value::String(e.to_string())),
                },
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
            Err(warp::reject::custom(NotFoundError(error)))
        }
    }
}

/// Create REST API routes
pub fn create_routes(
    registry: Arc<PlayerRegistry>,
    db_pool: Arc<PgPool>,
    snapshot_manager: Arc<SnapshotManager>,
    cache: Arc<CacheManager>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let registry_filter = warp::any().map(move || registry.clone());
    let db_pool_filter = warp::any().map(move || db_pool.clone());
    let snapshot_filter = warp::any().map(move || snapshot_manager.clone());
    let cache_filter = warp::any().map(move || cache.clone());

    // Symbol info endpoint
    let symbol_info = warp::path("api")
        .and(warp::path("symbol"))
        .and(warp::path::param::<u32>())
        .and(warp::path("info"))
        .and(warp::get())
        .and(registry_filter.clone())
        .and(cache_filter.clone())
        .and_then(
            |symbol_id: u32, registry: Arc<PlayerRegistry>, cache: Arc<CacheManager>| async move {
                get_symbol_info(symbol_id, registry, cache).await
            },
        );

    // All players endpoint for search
    let all_players = warp::path("api")
        .and(warp::path("symbols"))
        .and(warp::path("all"))
        .and(warp::get())
        .and(registry_filter.clone())
        .and_then(|registry: Arc<PlayerRegistry>| async move {
            get_all_players(registry).await
        });

    // Current price endpoint
    let current_price = warp::path("api")
        .and(warp::path("symbol"))
        .and(warp::path::param::<u32>())
        .and(warp::path("price"))
        .and(warp::get())
        .and(db_pool_filter.clone())
        .and_then(|symbol_id: u32, db_pool: Arc<PgPool>| async move {
            get_current_price(symbol_id, db_pool).await
        });

    // Bulk prices endpoint
    let bulk_prices = warp::path("api")
        .and(warp::path("symbols"))
        .and(warp::path("prices"))
        .and(warp::get())
        .and(db_pool_filter.clone())
        .and_then(|db_pool: Arc<PgPool>| async move {
            get_bulk_prices(db_pool).await
        });

    // Price history endpoint
    let price_history = warp::path("api")
        .and(warp::path("price-history"))
        .and(warp::path::param::<u32>())
        .and(warp::get())
        .and(warp::query::<PriceHistoryParams>())
        .and(db_pool_filter.clone())
        .and_then(|symbol_id: u32, params: PriceHistoryParams, db_pool: Arc<PgPool>| async move {
            get_price_history(symbol_id, params, db_pool).await
        });

    // Account summary endpoint
    let account_summary = warp::path("api")
        .and(warp::path("account"))
        .and(warp::path("summary"))
        .and(warp::get())
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(db_pool_filter.clone())
        .and_then(
            |params: std::collections::HashMap<String, String>, db_pool: Arc<PgPool>| async move {
                let account_id =
                    params.get("account_id").and_then(|s| s.parse::<i64>().ok()).unwrap_or(1); // Default to account 1 if not specified
                get_account_summary(account_id, db_pool).await
            },
        );

    // Equity history endpoint
    let equity_history = warp::path("api")
        .and(warp::path("account"))
        .and(warp::path("equity-history"))
        .and(warp::get())
        .and(warp::query::<EquityHistoryParams>())
        .and(db_pool_filter.clone())
        .and_then(get_equity_history);

    // Create snapshots endpoint (admin/test)
    let create_snapshots = warp::path("api")
        .and(warp::path("admin"))
        .and(warp::path("create-snapshots"))
        .and(warp::post())
        .and(db_pool_filter.clone())
        .and_then(create_daily_snapshots);

    // Test scheduler endpoint (admin/test)
    let test_scheduler = warp::path("api")
        .and(warp::path("admin"))
        .and(warp::path("test-scheduler"))
        .and(warp::post())
        .and(db_pool_filter.clone())
        .and_then(test_scheduler_logic);

    // Snapshot endpoint
    let snapshot = warp::path("api")
        .and(warp::path("snapshot"))
        .and(warp::path("current"))
        .and(warp::get())
        .and(snapshot_filter)
        .and_then(|snapshot_manager: Arc<SnapshotManager>| async move {
            get_current_snapshot(snapshot_manager).await
        });

    // Health check endpoint
    let health = warp::path("health").and(warp::get()).map(|| {
        warp::reply::json(&serde_json::json!({
            "status": "healthy",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    });

    // Combine all routes
    symbol_info
        .or(all_players)
        .or(current_price)
        .or(bulk_prices)
        .or(price_history)
        .or(account_summary)
        .or(equity_history)
        .or(create_snapshots)
        .or(test_scheduler)
        .or(snapshot)
        .or(health)
        .with(
            warp::cors()
                .allow_any_origin()
                .allow_headers(vec!["content-type"])
                .allow_methods(vec!["GET", "POST", "OPTIONS"]),
        )
}
