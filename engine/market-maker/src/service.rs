use crate::config::MarketMakerConfig;
use crate::models::*;
use crate::cache::FairPriceCache;
use anyhow::{Context, Result};
use sqlx::PgPool;
use std::collections::HashMap;
use std::path::PathBuf;
use persistence::snapshot::{SnapshotManager, Snapshot};
use persistence::config::SnapshotConfig;
use tracing::{info, warn, error, debug};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use url::Url;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Market Maker Service
pub struct MarketMakerService {
    config: MarketMakerConfig,
    db_pool: PgPool,
    cache: FairPriceCache,
    snapshot_manager: SnapshotManager,
    websocket_url: String,
    metrics: MarketMakerMetrics,
    symbol_to_player_mapping: HashMap<u32, i32>, // symbol_id -> sportsdataio_player_id
}

impl MarketMakerService {
    /// Create a new market maker service
    pub async fn new(config: MarketMakerConfig) -> Result<Self> {
        let db_pool = PgPool::connect(&config.database.url)
            .await
            .context("Failed to connect to database")?;
        
        let cache = FairPriceCache::new(config.cache.clone());
        
        // Create HTTP client
        let websocket_url = config.market_maker.websocket_gateway_url.clone();
        
        // Create snapshot manager
        let snapshot_config = SnapshotConfig {
            interval: std::time::Duration::from_secs(60), // 1 minute
            max_snapshots: 100,
            compress: false,
            snapshot_on_shutdown: false,
        };
        let snapshots_dir = PathBuf::from("data/snapshots");
        let snapshot_manager = SnapshotManager::new(snapshot_config, snapshots_dir)
            .context("Failed to create snapshot manager")?;
        
        let mut service = Self {
            config,
            db_pool,
            cache,
            snapshot_manager,
            websocket_url,
            metrics: MarketMakerMetrics::default(),
            symbol_to_player_mapping: HashMap::new(),
        };
        
        // Load symbol to player mapping
        service.load_symbol_mapping().await?;
        
        Ok(service)
    }
    
    /// Start the market maker service
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Market Maker service");
        
        if !self.config.market_maker.enabled {
            info!("Market Maker is disabled, exiting");
            return Ok(());
        }
        
        // Start the main market making loop
        self.run_market_making_loop().await
    }
    
    /// Main market making loop
    async fn run_market_making_loop(&mut self) -> Result<()> {
        let update_frequency = self.config.update_frequency();
        
        loop {
            match self.market_making_cycle().await {
                Ok(_) => {
                    self.metrics.last_update = Some(chrono::Utc::now());
                }
                Err(e) => {
                    error!("Market making cycle failed: {}", e);
                    self.metrics.errors += 1;
                }
            }
            
            sleep(update_frequency).await;
        }
    }
    
    /// Single market making cycle
    async fn market_making_cycle(&mut self) -> Result<()> {
        debug!("Starting market making cycle");
        
        // 1. Refresh fair prices if needed
        if self.cache.needs_refresh().await {
            self.refresh_fair_prices().await?;
            self.cache.mark_refreshed().await;
        }
        
        // 2. Get symbols that have recent fair prices
        let symbols_with_prices = self.get_symbols_with_fair_prices().await?;
        
        // 3. Load snapshot once for all symbols
        let snapshot = self.snapshot_manager.load_latest_snapshot().await?;
        
        // 4. Process each symbol with fair prices
        for symbol_id in symbols_with_prices {
            if let Err(e) = self.process_symbol_with_snapshot(symbol_id, &snapshot).await {
                warn!("Failed to process symbol {}: {}", symbol_id, e);
                self.metrics.errors += 1;
            }
        }
        
        // 4. Clean up expired cache entries
        self.cache.clear_expired().await;
        
        debug!("Completed market making cycle");
        Ok(())
    }
    
    /// Process a single symbol
    async fn process_symbol(&mut self, symbol_id: u32) -> Result<()> {
        self.metrics.order_book_checks += 1;
        
        // 1. Get fair price
        let fair_price = match self.get_fair_price(symbol_id).await? {
            Some(fp) => fp,
            None => {
                debug!("No fair price available for symbol {}", symbol_id);
                return Ok(());
            }
        };
        
        // 2. Get order book state
        let order_book = match self.get_order_book_state(symbol_id).await? {
            Some(ob) => ob,
            None => {
                debug!("No order book available for symbol {}", symbol_id);
                return Ok(());
            }
        };
        
        // 3. Make market making decision
        let decision = self.make_market_making_decision(&fair_price, &order_book);
        
        // 4. Execute decision
        self.execute_decision(decision).await?;
        
        Ok(())
    }
    
    /// Process a single symbol for market making with pre-loaded snapshot
    async fn process_symbol_with_snapshot(&mut self, symbol_id: u32, snapshot: &Option<Snapshot>) -> Result<()> {
        self.metrics.order_book_checks += 1;
        
        // 1. Get fair price
        let fair_price = match self.get_fair_price(symbol_id).await? {
            Some(fp) => fp,
            None => {
                debug!("No fair price available for symbol {}", symbol_id);
                return Ok(());
            }
        };
        
        // 2. Get order book state from pre-loaded snapshot
        let order_book = match snapshot {
            Some(snapshot) => {
                if let Some(persistence_order_book) = snapshot.state.order_books.get(&symbol_id) {
                    Some(OrderBookState {
                        symbol_id,
                        buy_orders: persistence_order_book.buy_orders.clone(),
                        sell_orders: persistence_order_book.sell_orders.clone(),
                        last_trade_price: persistence_order_book.last_trade_price,
                        last_trade_quantity: persistence_order_book.last_trade_quantity,
                        last_trade_timestamp: persistence_order_book.last_trade_timestamp,
                    })
                } else {
                    debug!("No order book found for symbol {} in snapshot", symbol_id);
                    None
                }
            }
            None => {
                debug!("No snapshots available for symbol {}", symbol_id);
                None
            }
        };
        
        // 3. Make market making decision
        let decision = match order_book {
            Some(ob) => self.make_market_making_decision(&fair_price, &ob),
            None => {
                // If no order book, post both quotes to activate the market
                debug!("No order book for symbol {}, posting both quotes to activate market", symbol_id);
                let spread_bps = self.config.market_maker.spread_bps;
                let quantity_bp = self.config.market_maker.order_quantity_bp;
                
                let bid = MarketMakerQuote::new(
                    symbol_id,
                    QuoteSide::Bid,
                    fair_price.fair_cents,
                    spread_bps,
                    quantity_bp,
                );
                let ask = MarketMakerQuote::new(
                    symbol_id,
                    QuoteSide::Ask,
                    fair_price.fair_cents,
                    spread_bps,
                    quantity_bp,
                );
                MarketMakerDecision::PostBoth { bid, ask }
            }
        };
        
        // 4. Execute decision
        self.execute_decision(decision).await?;
        
        Ok(())
    }
    
    /// Get fair price for a symbol
    async fn get_fair_price(&self, symbol_id: u32) -> Result<Option<FairPrice>> {
        // Try cache first
        if let Some(fair_price) = self.cache.get_fair_price(symbol_id).await {
            return Ok(Some(fair_price));
        }
        
        // Cache miss - fetch from database
        let player_id = match self.symbol_to_player_mapping.get(&symbol_id) {
            Some(pid) => *pid,
            None => {
                debug!("No player mapping found for symbol {}", symbol_id);
                return Ok(None);
            }
        };
        
        let row = sqlx::query!(
            r#"
            SELECT fair_cents, source, confidence_score, ts
            FROM rpe_fair_prices
            WHERE player_id = $1
            "#,
            player_id
        )
        .fetch_optional(&self.db_pool)
        .await
        .context("Failed to fetch fair price from database")?;
        
        let fair_price = match row {
            Some(row) => {
                let fair_price = FairPrice {
                    player_id,
                    symbol_id,
                    fair_cents: row.fair_cents,
                    source: row.source.unwrap_or_else(|| "unknown".to_string()),
                    confidence_score: row.confidence_score.unwrap_or_default(),
                    updated_at: row.ts,
                };
                
                // Store in cache for next time
                self.cache.store_fair_price(fair_price.clone()).await;
                Some(fair_price)
            }
            None => {
                debug!("No fair price found for player {} (symbol {})", player_id, symbol_id);
                None
            }
        };
        
        Ok(fair_price)
    }
    
    /// Get symbols that have fair prices (mapped to our internal symbol IDs)
    async fn get_symbols_with_fair_prices(&self) -> Result<Vec<u32>> {
        let rows = sqlx::query!(
            r#"
            SELECT DISTINCT pim.our_symbol_id
            FROM rpe_fair_prices rfp
            JOIN player_id_mapping pim ON rfp.player_id = pim.sportsdataio_player_id
            "#
        )
        .fetch_all(&self.db_pool)
        .await
        .context("Failed to fetch symbols with fair prices")?;
        
        let symbols: Vec<u32> = rows.into_iter().map(|row| row.our_symbol_id as u32).collect();
        info!("Found {} symbols with fair prices", symbols.len());
        Ok(symbols)
    }
    
    /// Get order book state for a symbol from latest snapshot
    async fn get_order_book_state(&self, symbol_id: u32) -> Result<Option<OrderBookState>> {
        match self.snapshot_manager.load_latest_snapshot().await? {
            Some(snapshot) => {
                if let Some(persistence_order_book) = snapshot.state.order_books.get(&symbol_id) {
                    // Convert persistence::OrderBookState to our OrderBookState
                    Ok(Some(OrderBookState {
                        symbol_id,
                        buy_orders: persistence_order_book.buy_orders.clone(),
                        sell_orders: persistence_order_book.sell_orders.clone(),
                        last_trade_price: persistence_order_book.last_trade_price,
                        last_trade_quantity: persistence_order_book.last_trade_quantity,
                        last_trade_timestamp: persistence_order_book.last_trade_timestamp,
                    }))
                } else {
                    debug!("No order book found for symbol {} in snapshot", symbol_id);
                    Ok(None)
                }
            }
            None => {
                debug!("No snapshots found, returning None for symbol {}", symbol_id);
                Ok(None)
            }
        }
    }
    
    /// Make market making decision based on fair price and order book
    fn make_market_making_decision(&self, fair_price: &FairPrice, order_book: &OrderBookState) -> MarketMakerDecision {
        let spread_bps = self.config.market_maker.spread_bps;
        let max_spread_bps = self.config.market_maker.max_spread_bps;
        let quantity_bp = self.config.market_maker.order_quantity_bp;
        
        // Check if order book is empty
        if order_book.is_empty() {
            debug!("Order book empty for symbol {}, posting both quotes", fair_price.symbol_id);
            let bid = MarketMakerQuote::new(
                fair_price.symbol_id,
                QuoteSide::Bid,
                fair_price.fair_cents,
                spread_bps,
                quantity_bp,
            );
            let ask = MarketMakerQuote::new(
                fair_price.symbol_id,
                QuoteSide::Ask,
                fair_price.fair_cents,
                spread_bps,
                quantity_bp,
            );
            return MarketMakerDecision::PostBoth { bid, ask };
        }
        
        // Check if spread is too wide
        if let Some(current_spread) = order_book.spread_bps() {
            if current_spread > max_spread_bps {
                debug!("Spread too wide for symbol {}: {} bps, posting both quotes", 
                       fair_price.symbol_id, current_spread);
                let bid = MarketMakerQuote::new(
                    fair_price.symbol_id,
                    QuoteSide::Bid,
                    fair_price.fair_cents,
                    spread_bps,
                    quantity_bp,
                );
                let ask = MarketMakerQuote::new(
                    fair_price.symbol_id,
                    QuoteSide::Ask,
                    fair_price.fair_cents,
                    spread_bps,
                    quantity_bp,
                );
                return MarketMakerDecision::PostBoth { bid, ask };
            }
        }
        
        // Check if one-sided
        if order_book.is_one_sided() {
            if order_book.buy_orders.is_empty() {
                debug!("No bids for symbol {}, posting bid", fair_price.symbol_id);
                let bid = MarketMakerQuote::new(
                    fair_price.symbol_id,
                    QuoteSide::Bid,
                    fair_price.fair_cents,
                    spread_bps,
                    quantity_bp,
                );
                return MarketMakerDecision::PostBid { bid };
            } else if order_book.sell_orders.is_empty() {
                debug!("No asks for symbol {}, posting ask", fair_price.symbol_id);
                let ask = MarketMakerQuote::new(
                    fair_price.symbol_id,
                    QuoteSide::Ask,
                    fair_price.fair_cents,
                    spread_bps,
                    quantity_bp,
                );
                return MarketMakerDecision::PostAsk { ask };
            }
        }
        
        // Do nothing - order book is healthy
        MarketMakerDecision::DoNothing
    }
    
    /// Execute market making decision
    async fn execute_decision(&mut self, decision: MarketMakerDecision) -> Result<()> {
        match decision {
            MarketMakerDecision::PostBoth { bid, ask } => {
                self.post_quote(bid).await?;
                self.post_quote(ask).await?;
            }
            MarketMakerDecision::PostBid { bid } => {
                self.post_quote(bid).await?;
            }
            MarketMakerDecision::PostAsk { ask } => {
                self.post_quote(ask).await?;
            }
            MarketMakerDecision::DoNothing => {
                // Nothing to do
            }
        }
        
        Ok(())
    }
    
    /// Post a market maker quote via WebSocket
    async fn post_quote(&mut self, quote: MarketMakerQuote) -> Result<()> {
        info!("Posting {} quote for symbol {}: {} cents, {} bp", 
              match quote.side {
                  QuoteSide::Bid => "BID",
                  QuoteSide::Ask => "ASK",
              },
              quote.symbol_id,
              quote.price_cents,
              quote.quantity_bp);
        
        // Connect to WebSocket
        let url = Url::parse(&self.websocket_url)
            .context("Failed to parse WebSocket URL")?;
        
        let (ws_stream, _) = connect_async(url)
            .await
            .context("Failed to connect to WebSocket")?;
        
        let (mut write, mut read) = ws_stream.split();
        
        // First, authenticate
        let auth_message = serde_json::json!({
            "method": "auth.login",
            "params": {
                "api_key": self.config.market_maker.api_key,
                "api_secret": self.config.market_maker.api_secret
            }
        });
        
        write.send(Message::Text(auth_message.to_string()))
            .await
            .context("Failed to send auth message")?;
        
        // Wait for auth response
        if let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let response: serde_json::Value = serde_json::from_str(&text)
                        .context("Failed to parse auth response")?;
                    
                    if response.get("result").and_then(|r| r.get("authenticated")).and_then(|a| a.as_bool()).unwrap_or(false) {
                        info!("✅ Market maker authenticated successfully");
                    } else {
                        error!("❌ Authentication failed: {}", text);
                        self.metrics.errors += 1;
                        return Ok(());
                    }
                }
                Ok(_) => {
                    error!("❌ Unexpected message type during authentication");
                    self.metrics.errors += 1;
                    return Ok(());
                }
                Err(e) => {
                    error!("❌ WebSocket error during authentication: {}", e);
                    self.metrics.errors += 1;
                    return Ok(());
                }
            }
        }
        
        // Now submit the order
        let order_message = serde_json::json!({
            "method": "order.place",
            "params": {
                "symbol": quote.symbol_id.to_string(),
                "side": match quote.side {
                    QuoteSide::Bid => "BUY",
                    QuoteSide::Ask => "SELL",
                },
                "type": "LIMIT",
                "quantity": quote.quantity_bp as u32,
                "price": quote.price_cents as u32,
            }
        });
        
        write.send(Message::Text(order_message.to_string()))
            .await
            .context("Failed to send order message")?;
        
        // Wait for order response
        if let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let response: serde_json::Value = serde_json::from_str(&text)
                        .context("Failed to parse order response")?;
                    
                    if response.get("result").is_some() {
                        info!("✅ Order submitted successfully: {}", text);
                        self.metrics.quotes_posted += 1;
                    } else if let Some(error) = response.get("error") {
                        error!("❌ Order submission failed: {}", error);
                        self.metrics.errors += 1;
                    } else {
                        error!("❌ Unexpected order response: {}", text);
                        self.metrics.errors += 1;
                    }
                }
                Ok(_) => {
                    error!("❌ Unexpected message type for order response");
                    self.metrics.errors += 1;
                }
                Err(e) => {
                    error!("❌ WebSocket error for order response: {}", e);
                    self.metrics.errors += 1;
                }
            }
        }
        
        Ok(())
    }
    
    /// Refresh fair prices from database
    async fn refresh_fair_prices(&mut self) -> Result<()> {
        info!("Refreshing fair prices from database");
        
        let rows = sqlx::query!(
            r#"
            SELECT 
                pim.our_symbol_id,
                rfp.player_id,
                rfp.fair_cents,
                rfp.source,
                rfp.confidence_score,
                rfp.ts
            FROM rpe_fair_prices rfp
            JOIN player_id_mapping pim ON rfp.player_id = pim.sportsdataio_player_id
            "#
        )
        .fetch_all(&self.db_pool)
        .await
        .context("Failed to fetch fair prices from database")?;
        
        let row_count = rows.len();
        for row in rows {
            let fair_price = FairPrice {
                player_id: row.player_id,
                symbol_id: row.our_symbol_id as u32,
                fair_cents: row.fair_cents,
                source: row.source.unwrap_or_else(|| "unknown".to_string()),
                confidence_score: row.confidence_score.unwrap_or_default(),
                updated_at: row.ts,
            };
            
            self.cache.store_fair_price(fair_price).await;
        }
        
        self.metrics.fair_price_updates += 1;
        info!("Refreshed {} fair prices", row_count);
        
        Ok(())
    }
    
    /// Load symbol to player mapping
    async fn load_symbol_mapping(&mut self) -> Result<()> {
        info!("Loading symbol to player mapping");
        
        let rows = sqlx::query!(
            "SELECT our_symbol_id, sportsdataio_player_id FROM player_id_mapping"
        )
        .fetch_all(&self.db_pool)
        .await
        .context("Failed to load symbol mapping")?;
        
        for row in rows {
            self.symbol_to_player_mapping.insert(row.our_symbol_id as u32, row.sportsdataio_player_id);
        }
        
        info!("Loaded {} symbol mappings", self.symbol_to_player_mapping.len());
        Ok(())
    }
    
    /// Get market maker metrics
    pub fn metrics(&self) -> &MarketMakerMetrics {
        &self.metrics
    }
    
    /// Get cache statistics
    pub async fn cache_stats(&self) -> crate::cache::CacheStats {
        self.cache.stats().await
    }
}
