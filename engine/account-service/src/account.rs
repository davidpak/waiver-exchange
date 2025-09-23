//! AccountService implementation

use crate::balance::Balance;
use crate::config::AccountServiceConfig;
use crate::oauth::GoogleOAuthClient;
use crate::position::{Position, TradeSide};
use crate::reservation::{Reservation, ReservationId, ReservationManager};
use crate::sleeper::{LeagueOption, SleeperClient};
use crate::trade::{Trade, TradeDetails};
use crate::{AccountServiceError, Result};
use bigdecimal::BigDecimal;
use chrono::Utc;
use redis::Client as RedisClient;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid;

/// Account represents a user account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub google_id: Option<String>,
    pub sleeper_user_id: Option<String>,
    pub sleeper_roster_id: Option<String>,
    pub sleeper_league_id: Option<String>,
    pub display_name: Option<String>,
    pub fantasy_points: Option<i32>,
    pub weekly_wins: Option<i32>,
    pub currency_balance: Option<i64>, // In cents
    pub created_at: Option<chrono::NaiveDateTime>,
    pub last_updated: Option<chrono::NaiveDateTime>,
}

/// AccountService provides account management, balance tracking, and risk validation
#[derive(Debug)]
pub struct AccountService {
    db_pool: PgPool,
    redis_client: RedisClient,
    sleeper_client: SleeperClient,
    google_client: GoogleOAuthClient,
    reservation_manager: Arc<Mutex<ReservationManager>>,
    config: AccountServiceConfig,
}

impl AccountService {
    /// Create a new AccountService
    pub async fn new(config: AccountServiceConfig) -> Result<Self> {
        // Create database pool
        let db_pool = PgPool::connect(&config.database.url).await?;

        // Run migrations
        sqlx::migrate!("./migrations").run(&db_pool).await?;

        // Create Redis client
        let redis_client = RedisClient::open(config.redis.url.clone())?;

        // Create Sleeper client
        let sleeper_client = SleeperClient::new(config.sleeper.clone());

        // Create Google OAuth client
        let google_client = GoogleOAuthClient::new(&config.oauth)?;

        Ok(Self {
            db_pool,
            redis_client,
            sleeper_client,
            google_client,
            reservation_manager: Arc::new(Mutex::new(ReservationManager::new())),
            config,
        })
    }

    /// Authenticate user with Google OAuth
    pub async fn authenticate_with_google(&self, google_token: &str) -> Result<Account> {
        let user_info = self.google_client.get_user_info(google_token).await?;

        // Check if account exists
        let account =
            sqlx::query_as!(Account, "SELECT * FROM accounts WHERE google_id = $1", user_info.id)
                .fetch_optional(&self.db_pool)
                .await?;

        if let Some(account) = account {
            Ok(account)
        } else {
            // Create new account
            let account = sqlx::query_as!(
                Account,
                "INSERT INTO accounts (google_id, display_name, created_at, last_updated) 
                 VALUES ($1, $2, NOW(), NOW()) 
                 RETURNING *",
                user_info.id,
                user_info.name
            )
            .fetch_one(&self.db_pool)
            .await?;

            Ok(account)
        }
    }

    /// Get or create account by Google user info (without calling Google API again)
    pub async fn get_or_create_account_by_google_info(
        &self,
        google_id: &str,
        email: &str,
        name: &str,
    ) -> Result<Account> {
        // Check if account exists
        let account =
            sqlx::query_as!(Account, "SELECT * FROM accounts WHERE google_id = $1", google_id)
                .fetch_optional(&self.db_pool)
                .await?;

        if let Some(account) = account {
            Ok(account)
        } else {
            // Create new account
            let account = sqlx::query_as!(
                Account,
                "INSERT INTO accounts (google_id, display_name, created_at, last_updated) 
                 VALUES ($1, $2, NOW(), NOW()) 
                 RETURNING *",
                google_id,
                name
            )
            .fetch_one(&self.db_pool)
            .await?;

            Ok(account)
        }
    }

    /// Get user's leagues (helper method)
    pub async fn get_user_leagues(&self, user_id: &str, season: &str) -> Result<Vec<LeagueOption>> {
        self.sleeper_client.get_user_leagues(user_id, season).await
    }

    /// Select a Sleeper league
    pub async fn select_sleeper_league(
        &self,
        account_id: i64,
        league_id: &str,
        roster_id: u32,
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE accounts SET sleeper_league_id = $1, sleeper_roster_id = $2, last_updated = NOW() 
             WHERE id = $3",
            league_id,
            roster_id.to_string(), // Convert u32 to string for database storage
            account_id
        )
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    /// Get account ID by user ID (Google ID or Sleeper user ID)
    pub async fn get_account_id_by_user_id(&self, user_id: &str) -> Result<i64> {
        // Try to find by Google ID first
        if let Ok(account) =
            sqlx::query_as!(Account, "SELECT * FROM accounts WHERE google_id = $1", user_id)
                .fetch_one(&self.db_pool)
                .await
        {
            return Ok(account.id);
        }

        // Try to find by Sleeper user ID
        if let Ok(account) =
            sqlx::query_as!(Account, "SELECT * FROM accounts WHERE sleeper_user_id = $1", user_id)
                .fetch_one(&self.db_pool)
                .await
        {
            return Ok(account.id);
        }

        Err(AccountServiceError::AccountNotFound { account_id: 0 })
    }

    /// Get account by ID
    pub async fn get_account(&self, account_id: i64) -> Result<Account> {
        let account = sqlx::query_as!(Account, "SELECT * FROM accounts WHERE id = $1", account_id)
            .fetch_optional(&self.db_pool)
            .await?
            .ok_or_else(|| AccountServiceError::AccountNotFound { account_id })?;

        Ok(account)
    }

    /// Get account balance in cents
    pub async fn get_balance(&self, account_id: i64) -> Result<i64> {
        let row = sqlx::query!("SELECT currency_balance FROM accounts WHERE id = $1", account_id)
            .fetch_optional(&self.db_pool)
            .await?
            .ok_or_else(|| AccountServiceError::AccountNotFound { account_id })?;

        Ok(row.currency_balance.unwrap_or(0))
    }

    /// Get all positions for an account
    pub async fn get_positions(&self, account_id: i64) -> Result<Vec<Position>> {
        let rows = sqlx::query!(
            "SELECT symbol_id, quantity, avg_cost, last_updated FROM positions WHERE account_id = $1",
            account_id
        )
        .fetch_all(&self.db_pool)
        .await?;

        let positions = rows
            .into_iter()
            .map(|row| Position {
                account_id,
                symbol_id: row.symbol_id,
                quantity: Balance::from_basis_points(row.quantity),
                avg_cost: Balance::from_cents(row.avg_cost),
                last_updated: row.last_updated.unwrap_or_else(|| chrono::Utc::now().naive_utc()),
            })
            .collect();

        Ok(positions)
    }

    /// Get position for a specific symbol
    pub async fn get_position(&self, account_id: i64, symbol_id: i64) -> Result<Option<Position>> {
        let row = sqlx::query!(
            "SELECT symbol_id, quantity, avg_cost, last_updated FROM positions WHERE account_id = $1 AND symbol_id = $2",
            account_id,
            symbol_id
        )
        .fetch_optional(&self.db_pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(Position {
                account_id,
                symbol_id: row.symbol_id,
                quantity: Balance::from_basis_points(row.quantity),
                avg_cost: Balance::from_cents(row.avg_cost),
                last_updated: row.last_updated.unwrap_or_else(|| chrono::Utc::now().naive_utc()),
            }))
        } else {
            Ok(None)
        }
    }

    /// Check and reserve balance for an order
    pub async fn check_and_reserve_balance(
        &self,
        account_id: i64,
        amount: i64,
        order_id: i64,
    ) -> Result<ReservationId> {
        // Get current balance
        let balance = self.get_balance(account_id).await?;

        // Get total reserved amount
        let mut manager = self.reservation_manager.lock().await;
        let total_reserved = manager.get_total_reserved(account_id);

        // Check if sufficient balance is available
        let available_balance = balance - total_reserved.to_cents();
        if available_balance < amount {
            return Err(AccountServiceError::InsufficientBalance {
                required: amount as u64,
                available: available_balance.max(0) as u64,
            });
        }

        // Create reservation with unique ID (mask high bit to prevent negative numbers)
        let reservation_id =
            ReservationId(uuid::Uuid::new_v4().as_u128() as u64 & 0x7FFFFFFFFFFFFFFF);
        let expires_at =
            Utc::now() + chrono::Duration::days(self.config.reservation_expiry_days as i64);
        let reservation = Reservation::new(
            reservation_id,
            account_id,
            Balance::from_cents(amount),
            order_id,
            expires_at.naive_utc(),
        );

        // Store reservation in database
        sqlx::query!(
            "INSERT INTO reservations (id, account_id, amount, order_id, status, created_at, expires_at) 
             VALUES ($1, $2, $3, $4, $5, NOW(), $6)",
            reservation_id.0 as i64,
            account_id,
            amount,
            order_id,
            "active",
            expires_at.naive_utc()
        )
        .execute(&self.db_pool)
        .await?;

        // Add to in-memory manager
        manager.add_reservation(reservation);

        Ok(reservation_id)
    }

    /// Settle a reserved balance (convert reservation to actual trade)
    pub async fn settle_reserved_balance(
        &self,
        reservation_id: ReservationId,
        trade_details: TradeDetails,
    ) -> Result<()> {
        // Get reservation from database
        let row = sqlx::query!(
            "SELECT * FROM reservations WHERE id = $1 AND status = 'active'",
            reservation_id.0 as i64
        )
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or_else(|| AccountServiceError::ReservationNotFound {
            reservation_id: reservation_id.0,
        })?;

        // Update reservation status to settled
        sqlx::query!(
            "UPDATE reservations SET status = 'settled' WHERE id = $1",
            reservation_id.0 as i64
        )
        .execute(&self.db_pool)
        .await?;

        // Settle the trade
        self.settle_trade(&trade_details).await?;

        // Remove from in-memory manager
        let mut manager = self.reservation_manager.lock().await;
        manager.remove_reservation(reservation_id);

        Ok(())
    }

    /// Release a reservation (cancel order)
    pub async fn release_reservation(&self, reservation_id: ReservationId) -> Result<()> {
        // Update reservation status in database
        sqlx::query!(
            "UPDATE reservations SET status = 'cancelled' WHERE id = $1",
            reservation_id.0 as i64
        )
        .execute(&self.db_pool)
        .await?;

        // Remove from in-memory manager
        let mut manager = self.reservation_manager.lock().await;
        manager.remove_reservation(reservation_id);

        Ok(())
    }

    /// Settle a trade (update balances and positions)
    pub async fn settle_trade(&self, trade_details: &TradeDetails) -> Result<()> {
        // Update account balance
        let cash_impact = match trade_details.side {
            TradeSide::Buy => -(trade_details.quantity.to_cents() * trade_details.price.to_cents()),
            TradeSide::Sell => trade_details.quantity.to_cents() * trade_details.price.to_cents(),
        };

        sqlx::query!(
            "UPDATE accounts SET currency_balance = currency_balance + $1, last_updated = NOW() WHERE id = $2",
            cash_impact,
            trade_details.account_id
        )
        .execute(&self.db_pool)
        .await?;

        // Update or create position
        let existing_position =
            self.get_position(trade_details.account_id, trade_details.symbol_id).await?;

        if let Some(mut position) = existing_position {
            // Update existing position
            position.update_with_trade(
                trade_details.side,
                trade_details.quantity,
                trade_details.price,
            );

            sqlx::query!(
                "UPDATE positions SET quantity = $1, avg_cost = $2, last_updated = NOW() 
                 WHERE account_id = $3 AND symbol_id = $4",
                position.quantity.to_cents(),
                position.avg_cost.to_cents(),
                trade_details.account_id,
                trade_details.symbol_id
            )
            .execute(&self.db_pool)
            .await?;
        } else {
            // Create new position
            let position = Position::new(
                trade_details.account_id,
                trade_details.symbol_id,
                trade_details.quantity,
                trade_details.price,
            );

            sqlx::query!(
                "INSERT INTO positions (account_id, symbol_id, quantity, avg_cost, last_updated) 
                 VALUES ($1, $2, $3, $4, NOW())",
                trade_details.account_id,
                trade_details.symbol_id,
                position.quantity.to_cents(),
                position.avg_cost.to_cents()
            )
            .execute(&self.db_pool)
            .await?;
        }

        // Record trade in history
        sqlx::query!(
            "INSERT INTO trades (account_id, symbol_id, side, quantity, price, order_id, timestamp) 
             VALUES ($1, $2, $3, $4, $5, $6, NOW())",
            trade_details.account_id,
            trade_details.symbol_id,
            format!("{:?}", trade_details.side),
            trade_details.quantity.to_cents(),
            trade_details.price.to_cents(),
            trade_details.order_id
        )
        .execute(&self.db_pool)
        .await?;

        // Record price history for this trade
        self.record_price_history(
            trade_details.symbol_id as i32,
            trade_details.price.to_cents(),
            trade_details.quantity.to_cents(),
        )
        .await?;

        Ok(())
    }

    /// Record price history for a trade
    async fn record_price_history(&self, symbol_id: i32, price: i64, quantity: i64) -> Result<()> {
        let timestamp = Utc::now().naive_utc();

        // For now, create a simple candle for each trade
        // In production, this would aggregate trades into time-based candles (5min, 1hour, etc.)
        sqlx::query!(
            r#"
            INSERT INTO price_history (symbol_id, timestamp, open_price, high_price, low_price, close_price, volume)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (symbol_id, timestamp) DO UPDATE SET
                high_price = GREATEST(price_history.high_price, $4),
                low_price = LEAST(price_history.low_price, $5),
                close_price = $6,
                volume = price_history.volume + $7
            "#,
            symbol_id,
            timestamp,
            price, // open_price
            price, // high_price
            price, // low_price
            price, // close_price
            quantity // volume
        )
        .execute(&self.db_pool)
        .await?;

        tracing::debug!(
            "Recorded price history for symbol {}: price={}, quantity={}, timestamp={:?}",
            symbol_id,
            price,
            quantity,
            timestamp
        );

        Ok(())
    }

    /// Get trade history for an account
    pub async fn get_trade_history(
        &self,
        account_id: i64,
        limit: Option<u32>,
    ) -> Result<Vec<Trade>> {
        let limit = limit.unwrap_or(100) as i64;

        let rows = sqlx::query!(
            "SELECT id, symbol_id, side, quantity, price, order_id, timestamp 
             FROM trades WHERE account_id = $1 ORDER BY timestamp DESC LIMIT $2",
            account_id,
            limit
        )
        .fetch_all(&self.db_pool)
        .await?;

        let trades = rows
            .into_iter()
            .map(|row| {
                let side = match row.side.as_str() {
                    "Buy" => TradeSide::Buy,
                    "Sell" => TradeSide::Sell,
                    _ => TradeSide::Buy, // Default fallback
                };

                Trade {
                    id: row.id,
                    account_id,
                    symbol_id: row.symbol_id,
                    side,
                    quantity: Balance::from_basis_points(row.quantity),
                    price: Balance::from_cents(row.price),
                    timestamp: row.timestamp.unwrap_or_else(|| chrono::Utc::now().naive_utc()),
                    order_id: row.order_id,
                }
            })
            .collect();

        Ok(trades)
    }

    /// Update fantasy points and wins for an account
    pub async fn update_fantasy_points_and_wins(&self, account_id: i64) -> Result<()> {
        let account = self.get_account(account_id).await?;

        if let (Some(league_id), Some(roster_id)) =
            (&account.sleeper_league_id, &account.sleeper_roster_id)
        {
            // Get season total fantasy points
            let season_points = self.sleeper_client.get_season_points(league_id, roster_id).await?;
            println!(
                "DEBUG: Account {} - season_points: {}, conversion_rate: {}",
                account_id, season_points, self.config.fantasy_points_conversion_rate
            );

            // Calculate weekly bonuses (simplified - would need actual implementation)
            let total_bonus = 0; // Placeholder

            // Convert to currency: (season_points + bonuses) * conversion_rate
            let total_currency = (season_points as i64 + total_bonus as i64)
                * self.config.fantasy_points_conversion_rate as i64;
            println!(
                "DEBUG: Account {} - total_currency calculation: {} * {} = {}",
                account_id,
                season_points,
                self.config.fantasy_points_conversion_rate,
                total_currency
            );

            // Update account
            sqlx::query!(
                "UPDATE accounts SET fantasy_points = $1, currency_balance = $2, last_updated = NOW() WHERE id = $3",
                season_points as i32,
                total_currency as i64,
                account_id
            )
            .execute(&self.db_pool)
            .await?;
        }

        Ok(())
    }

    /// Clean up expired reservations
    pub async fn cleanup_expired_reservations(&self) -> Result<()> {
        // Update expired reservations in database
        sqlx::query!(
            "UPDATE reservations SET status = 'expired' WHERE status = 'active' AND expires_at < NOW()"
        )
        .execute(&self.db_pool)
        .await?;

        // Clean up in-memory manager
        let mut manager = self.reservation_manager.lock().await;
        manager.cleanup_expired();

        Ok(())
    }

    /// Health check
    pub async fn health_check(&self) -> Result<()> {
        // Check database connectivity
        sqlx::query!("SELECT 1 as test").fetch_one(&self.db_pool).await?;

        // Check Redis connectivity
        let mut conn = self.redis_client.get_async_connection().await?;
        redis::cmd("PING").query_async::<_, String>(&mut conn).await?;

        // Check Sleeper API
        self.sleeper_client.health_check().await?;

        Ok(())
    }

    /// Set up sleeper integration for an account using username
    pub async fn setup_sleeper_integration(
        &self,
        account_id: i64,
        sleeper_username: &str,
    ) -> Result<Vec<LeagueOption>> {
        // First, get the current NFL state to determine the active season
        let nfl_state = self.sleeper_client.get_nfl_state().await?;
        println!(
            "DEBUG: Current NFL state - season: {}, league_season: {}",
            nfl_state.season, nfl_state.league_season
        );

        // Use league_season as it's the active season for leagues
        let current_season = &nfl_state.league_season;

        // Get the sleeper user ID from the username
        let sleeper_user_id = self.sleeper_client.get_user_id(sleeper_username).await?;
        println!("DEBUG: Found user_id {} for username {}", sleeper_user_id, sleeper_username);

        // Update the account with the user_id
        sqlx::query!(
            "UPDATE accounts SET sleeper_user_id = $1, last_updated = NOW() WHERE id = $2",
            sleeper_user_id,
            account_id
        )
        .execute(&self.db_pool)
        .await?;

        // Get user's leagues for the current active season
        let mut leagues =
            self.sleeper_client.get_user_leagues(&sleeper_user_id, current_season).await?;

        // If no leagues found for current season, try previous season
        if leagues.is_empty() {
            println!(
                "DEBUG: No leagues found for season {}, trying previous season {}",
                current_season, nfl_state.previous_season
            );
            leagues = self
                .sleeper_client
                .get_user_leagues(&sleeper_user_id, &nfl_state.previous_season)
                .await?;
        }

        // If still no leagues, try a few more recent seasons
        if leagues.is_empty() {
            let recent_seasons = ["2024", "2023", "2022"];
            for season in &recent_seasons {
                if season != current_season && season != &nfl_state.previous_season {
                    println!("DEBUG: Trying season {}", season);
                    leagues =
                        self.sleeper_client.get_user_leagues(&sleeper_user_id, season).await?;
                    if !leagues.is_empty() {
                        break;
                    }
                }
            }
        }

        Ok(leagues)
    }

    /// Check if account has sleeper integration set up
    pub async fn has_sleeper_integration(&self, account_id: i64) -> Result<bool> {
        let row = sqlx::query!("SELECT sleeper_user_id FROM accounts WHERE id = $1", account_id)
            .fetch_optional(&self.db_pool)
            .await?;

        Ok(row.map_or(false, |r| r.sleeper_user_id.is_some()))
    }

    /// Create daily equity snapshots for all accounts
    pub async fn create_daily_equity_snapshots(&self) -> Result<()> {
        let today = chrono::Utc::now().date_naive();

        // Get all accounts
        let accounts = sqlx::query!("SELECT id FROM accounts").fetch_all(&self.db_pool).await?;

        let mut snapshots = Vec::new();

        for account in &accounts {
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
                    let percent_f64 = (change as f64 / prev.total_equity as f64) * 100.0;
                    BigDecimal::from_str(&format!("{:.4}", percent_f64))
                        .unwrap_or_else(|_| BigDecimal::from(0))
                } else {
                    BigDecimal::from(0)
                };
                (change, percent)
            } else {
                (0, BigDecimal::from(0)) // First day, no change
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
        for (
            account_id,
            date,
            total_equity,
            cash_balance,
            position_value,
            day_change,
            day_change_percent,
        ) in snapshots
        {
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

    /// Calculate total equity for an account (matches existing logic from rest_api.rs)
    async fn calculate_total_equity(&self, account_id: i64, cash_balance: u64) -> u64 {
        // Get all positions for this account
        match sqlx::query!(
            "SELECT symbol_id, quantity, avg_cost FROM positions WHERE account_id = $1",
            account_id
        )
        .fetch_all(&self.db_pool)
        .await
        {
            Ok(positions) => {
                let mut total_position_value = 0i64; // Use i64 to handle negative values

                for position in positions {
                    let quantity = position.quantity as i64; // Keep as i64 to handle negative quantities
                    if quantity != 0 {
                        // Process both long and short positions
                        // Get current price for this symbol from price_history
                        let current_price = match sqlx::query!(
                            "SELECT close_price FROM price_history 
                             WHERE symbol_id = $1 
                             ORDER BY timestamp DESC 
                             LIMIT 1",
                            position.symbol_id as i32
                        )
                        .fetch_optional(&self.db_pool)
                        .await
                        {
                            Ok(Some(row)) => row.close_price as u64,
                            Ok(None) => position.avg_cost as u64, // Fallback to avg_cost if no price history
                            Err(_) => position.avg_cost as u64,   // Fallback to avg_cost on error
                        };

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
            Err(_) => cash_balance, // If we can't get positions, just return cash balance
        }
    }
}
