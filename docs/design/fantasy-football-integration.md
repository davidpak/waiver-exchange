> **Implementation Status:** Implemented. Player-to-symbol mapping with deterministic hashing and linear probing for collisions is working. Player data is loaded from pre-scraped JSON files via `player-scraper` crate, not from live Sleeper API calls as described in section 7. Real-time webhook updates are not implemented.

---

# Fantasy Football Integration Design

## 1. Overview

The fantasy football integration transforms the trading system into a platform where users can trade shares of NFL players. This design leverages the [Sleeper API](https://docs.sleeper.com/#fetch-all-players) to fetch real player data and creates a consistent mapping between NFL players and trading symbols.

## 2. Architecture Overview

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Sleeper API   │    │   Player        │    │   Trading       │
│                 │    │   Mapping       │    │   System        │
│  • Player Data  │───▶│  • Symbol ID    │───▶│  • Order Books  │
│  • Team Info    │    │  • Metadata     │    │  • Market Data  │
│  • Position     │    │  • Consistency  │    │  • Real-time    │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## 3. Data Source: Sleeper API

### 3.1 API Endpoint

**Fetch All Players**: `GET https://api.sleeper.app/v1/players/nfl`

**Response Format**:
```json
{
  "3086": {
    "hashtag": "#TomBrady-NFL-NE-12",
    "depth_chart_position": 1,
    "status": "Active",
    "sport": "nfl",
    "fantasy_positions": ["QB"],
    "number": 12,
    "search_last_name": "brady",
    "injury_start_date": null,
    "weight": "220",
    "position": "QB",
    "practice_participation": null,
    "sportradar_id": "",
    "team": "NE",
    "last_name": "Brady",
    "college": "Michigan",
    "fantasy_data_id": 17836,
    "injury_status": null,
    "player_id": "3086",
    "height": "6'4\"",
    "search_full_name": "tombrady",
    "age": 40,
    "stats_id": "",
    "birth_country": "United States",
    "espn_id": "",
    "search_rank": 24,
    "first_name": "Tom",
    "depth_chart_order": 1,
    "years_exp": 14,
    "rotowire_id": null,
    "rotoworld_id": 8356,
    "search_first_name": "tom",
    "yahoo_id": null
  }
}
```

### 3.2 Data Usage

**Player Selection Criteria**:
- **Active players only** - Status = "Active"
- **NFL players only** - Sport = "nfl"
- **All positions** - QB, RB, WR, TE, K, DEF
- **Estimated count** - ~500 active players

**Key Fields Used**:
- `player_id` - Unique identifier
- `first_name`, `last_name` - Display name
- `team` - Current team
- `position` - Player position
- `status` - Active/Inactive status
- `number` - Jersey number

## 4. Player-to-Symbol Mapping

### 4.1 Symbol Assignment Strategy

**Consistent Hashing**: Use player ID hash to ensure same player always gets same symbol

```rust
pub struct PlayerSymbol {
    pub player_id: String,        // From Sleeper API
    pub symbol_id: u32,           // Our internal symbol ID (1-1000)
    pub player_name: String,      // "Tom Brady"
    pub team: String,             // "NE"
    pub position: String,         // "QB"
    pub jersey_number: u32,       // 12
    pub sleeper_data: PlayerData, // Full Sleeper player object
}

// Consistent symbol assignment
fn assign_symbol_to_player(player_id: &str) -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    player_id.hash(&mut hasher);
    let hash = hasher.finish();
    
    // Map to symbol range 1-1000
    (hash % 1000) + 1
}
```

### 4.2 Symbol ID Ranges

**Symbol Allocation**:
- **Symbols 1-500**: Active NFL players
- **Symbols 501-1000**: Reserved for future expansion
- **Symbols 1001+**: Reserved for teams, leagues, etc.

**Benefits**:
- **Consistency**: Same player always gets same symbol
- **Scalability**: Room for 1000+ symbols
- **Organization**: Logical grouping of symbol types

## 5. Player Data Management

### 5.1 Player Registry

```rust
pub struct PlayerRegistry {
    player_symbols: HashMap<String, u32>,      // player_id -> symbol_id
    symbol_players: HashMap<u32, PlayerSymbol>, // symbol_id -> player_data
    active_players: HashSet<String>,           // Set of active player IDs
    team_players: HashMap<String, Vec<u32>>,   // team -> list of symbol_ids
    position_players: HashMap<String, Vec<u32>>, // position -> list of symbol_ids
}

impl PlayerRegistry {
    pub fn new() -> Self {
        Self {
            player_symbols: HashMap::new(),
            symbol_players: HashMap::new(),
            active_players: HashSet::new(),
            team_players: HashMap::new(),
            position_players: HashMap::new(),
        }
    }
    
    pub async fn initialize_from_sleeper(&mut self) -> Result<()> {
        let players = fetch_sleeper_players().await?;
        
        for (player_id, player_data) in players {
            // Only include active players
            if player_data.status == "Active" {
                let symbol_id = assign_symbol_to_player(&player_id);
                let player_symbol = PlayerSymbol {
                    player_id: player_id.clone(),
                    symbol_id,
                    player_name: format!("{} {}", player_data.first_name, player_data.last_name),
                    team: player_data.team,
                    position: player_data.position,
                    jersey_number: player_data.number.parse().unwrap_or(0),
                    sleeper_data: player_data,
                };
                
                // Add to registry
                self.player_symbols.insert(player_id.clone(), symbol_id);
                self.symbol_players.insert(symbol_id, player_symbol);
                self.active_players.insert(player_id);
                
                // Add to team and position indexes
                self.team_players.entry(player_data.team.clone())
                    .or_insert_with(Vec::new)
                    .push(symbol_id);
                self.position_players.entry(player_data.position.clone())
                    .or_insert_with(Vec::new)
                    .push(symbol_id);
            }
        }
        
        Ok(())
    }
}
```

### 5.2 Order Book Initialization

```rust
impl PlayerRegistry {
    pub fn initialize_order_books(&self, system_state: &mut SystemState) -> Result<()> {
        for (symbol_id, player_symbol) in &self.symbol_players {
            // Create Whistle engine for this player
            let cfg = whistle::Config {
                symbol: *symbol_id,
                batch_max: 1024,
                arena_capacity: 4096,
                elastic_arena: false,
                exec_shift_bits: 12,
                exec_id_mode: whistle::ExecIdMode::Sharded,
                self_match_policy: whistle::SelfMatchPolicy::Skip,
                allow_market_cold_start: false,
                reference_price_source: whistle::ReferencePriceSource::SnapshotLastTrade,
            };
            
            let engine = Whistle::new(cfg);
            system_state.add_engine(*symbol_id, engine);
            
            // Initialize market data
            let market_data = MarketData {
                symbol: *symbol_id,
                player_name: player_symbol.player_name.clone(),
                team: player_symbol.team.clone(),
                position: player_symbol.position.clone(),
                jersey_number: player_symbol.jersey_number,
                last_trade_price: None,
                last_trade_qty: None,
                last_trade_time: None,
                bid_price: None,
                ask_price: None,
                bid_qty: None,
                ask_qty: None,
                trades: Vec::new(),
                book_deltas: Vec::new(),
            };
            
            system_state.add_market_data(*symbol_id, market_data);
        }
        
        Ok(())
    }
}
```

## 6. Enhanced Market Data

### 6.1 Player-Specific Market Data

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerMarketData {
    // Standard market data
    pub symbol: u32,
    pub last_trade_price: Option<Price>,
    pub last_trade_qty: Option<Qty>,
    pub last_trade_time: Option<TickId>,
    pub bid_price: Option<Price>,
    pub ask_price: Option<Price>,
    pub bid_qty: Option<Qty>,
    pub ask_qty: Option<Qty>,
    pub trades: Vec<Trade>,
    pub book_deltas: Vec<BookDelta>,
    
    // Player-specific data
    pub player_name: String,
    pub team: String,
    pub position: String,
    pub jersey_number: u32,
    pub sleeper_data: PlayerData,
    
    // Trading statistics
    pub volume_24h: Qty,
    pub high_24h: Option<Price>,
    pub low_24h: Option<Price>,
    pub change_24h: Option<Price>,
    pub change_percent_24h: Option<f64>,
}
```

### 6.2 Market Data Display

**Order Book Display with Player Info**:
```
🎯 TOM BRADY (QB, NE #12) - Symbol 1
📈 Current Price: $150.00 (+5.2%)
📊 24h Volume: 1,250 shares
📊 24h High: $155.00 | Low: $145.00

📚 Order Book:
Price | Amount | Total (Top 10 Sells)
155   | 25     | 3,875
154   | 15     | 2,310
153   | 20     | 3,060

Last Trade: $150.00 @ 10 shares

Price | Amount | Total (Top 10 Buys)
150   | 30     | 4,500
149   | 25     | 3,725
148   | 20     | 2,960
```

## 7. API Integration

### 7.1 Sleeper API Client

```rust
pub struct SleeperApiClient {
    base_url: String,
    client: reqwest::Client,
    cache: HashMap<String, PlayerData>,
    last_fetch: Option<SystemTime>,
}

impl SleeperApiClient {
    pub fn new() -> Self {
        Self {
            base_url: "https://api.sleeper.app/v1".to_string(),
            client: reqwest::Client::new(),
            cache: HashMap::new(),
            last_fetch: None,
        }
    }
    
    pub async fn fetch_all_players(&mut self) -> Result<HashMap<String, PlayerData>> {
        // Check if we need to refresh (once per day)
        if let Some(last_fetch) = self.last_fetch {
            if SystemTime::now().duration_since(last_fetch).unwrap() < Duration::from_secs(86400) {
                return Ok(self.cache.clone());
            }
        }
        
        let url = format!("{}/players/nfl", self.base_url);
        let response = self.client.get(&url).send().await?;
        let players: HashMap<String, PlayerData> = response.json().await?;
        
        // Update cache
        self.cache = players.clone();
        self.last_fetch = Some(SystemTime::now());
        
        Ok(players)
    }
    
    pub async fn fetch_trending_players(&self, trend_type: &str) -> Result<Vec<TrendingPlayer>> {
        let url = format!("{}/players/nfl/trending/{}", self.base_url, trend_type);
        let response = self.client.get(&url).send().await?;
        let trending: Vec<TrendingPlayer> = response.json().await?;
        
        Ok(trending)
    }
}
```

### 7.2 Data Refresh Strategy

**Refresh Schedule**:
- **Player data**: Once per day (5MB download)
- **Trending players**: Every hour
- **Injury updates**: Every 4 hours
- **Team changes**: Real-time via webhooks (future)

**Caching Strategy**:
- **Local cache**: Store player data in memory
- **File cache**: Save to disk for offline access
- **TTL**: 24 hours for player data, 1 hour for trending

## 8. Trading Scenarios

### 8.1 Fantasy Football Trading

**Player Shares**: Users buy/sell shares of individual players
**Team Exposure**: Users can build portfolios of players
**Position Diversification**: Users can trade across positions
**Injury Impact**: Player injuries affect share prices

**Example Trading Scenarios**:
```
User A: Buy 100 shares of Tom Brady @ $150
User B: Sell 50 shares of Tom Brady @ $149
User C: Buy 200 shares of Patrick Mahomes @ $200
User D: Sell 75 shares of Aaron Rodgers @ $180
```

### 8.2 Market Dynamics

**Price Drivers**:
- **Performance**: Good games increase demand
- **Injuries**: Injuries decrease demand
- **Team changes**: Trades affect value
- **Season context**: Playoff implications
- **News**: Media coverage impacts sentiment

**Trading Patterns**:
- **Game days**: Higher volatility
- **Injury news**: Sharp price movements
- **Trade deadlines**: Increased activity
- **Playoff races**: Position-specific demand

## 9. User Interface Enhancements

### 9.1 Player Search and Discovery

**Search Features**:
- **Player name**: "Tom Brady", "Brady"
- **Team**: "NE", "Patriots"
- **Position**: "QB", "Quarterback"
- **Jersey number**: "12"

**Filtering Options**:
- **By team**: Show all Patriots players
- **By position**: Show all QBs
- **By price range**: Show players under $100
- **By volume**: Show most traded players

### 9.2 Player Information Display

**Player Profile**:
```
👤 TOM BRADY
🏈 QB | New England Patriots | #12
💰 Current Price: $150.00
📊 24h Change: +5.2% (+$7.50)
📈 24h Volume: 1,250 shares
📊 24h High: $155.00 | Low: $145.00

📋 Player Stats:
• Age: 40
• Experience: 14 years
• College: Michigan
• Height: 6'4"
• Weight: 220 lbs

📰 Recent News:
• "Brady leads Patriots to victory"
• "Injury status: Probable"
```

## 10. Configuration

### 10.1 Player Integration Config

```toml
[player_integration]
sleeper_api_enabled = true
refresh_interval_hours = 24
cache_duration_hours = 24
max_players = 500
symbol_range_start = 1
symbol_range_end = 1000

[player_integration.sleeper]
base_url = "https://api.sleeper.app/v1"
timeout_seconds = 30
retry_attempts = 3
rate_limit_per_minute = 1000

[player_integration.filtering]
active_players_only = true
include_injured = true
include_suspended = false
min_games_played = 0
```

### 10.2 Symbol Assignment Config

```toml
[symbol_assignment]
hash_algorithm = "default"  # "default", "md5", "sha256"
symbol_range = [1, 1000]
reserved_ranges = [
    { start = 501, end = 1000, purpose = "future_expansion" },
    { start = 1001, end = 2000, purpose = "teams" },
    { start = 2001, end = 3000, purpose = "leagues" }
]
```

## 11. Testing Strategy

### 11.1 Unit Tests

- **Symbol assignment consistency**
- **Player data parsing**
- **API client functionality**
- **Market data initialization**

### 11.2 Integration Tests

- **Sleeper API integration**
- **Player registry initialization**
- **Order book creation**
- **Market data updates**

### 11.3 End-to-End Tests

- **Complete player trading flow**
- **Data refresh scenarios**
- **Error handling and recovery**
- **Performance under load**

## 12. Monitoring and Observability

### 12.1 Metrics

- **Player count**: Active players loaded
- **API calls**: Sleeper API request count
- **Data freshness**: Time since last update
- **Symbol assignment**: Distribution of symbols
- **Trading volume**: Per-player trading activity

### 12.2 Health Checks

- **Sleeper API connectivity**
- **Player data freshness**
- **Symbol assignment consistency**
- **Order book initialization**

### 12.3 Alerts

- **API failures**: Sleeper API down
- **Data staleness**: Player data > 25 hours old
- **Symbol conflicts**: Duplicate symbol assignments
- **Missing players**: Expected players not loaded

## 13. Future Enhancements

### 13.1 Advanced Features

- **Real-time injury updates**: Webhook integration
- **Player statistics**: Historical performance data
- **Team news integration**: News impact on prices
- **Social sentiment**: Twitter/Reddit sentiment analysis

### 13.2 Trading Features

- **Player futures**: Season-long contracts
- **Team portfolios**: Trade entire team rosters
- **Position limits**: Maximum exposure per position
- **Risk management**: Portfolio diversification tools

### 13.3 Data Enhancements

- **Historical data**: Past season performance
- **Projections**: Fantasy point projections
- **Injury reports**: Detailed injury information
- **Weather data**: Game day weather impact

---

This fantasy football integration design transforms the trading system into a comprehensive platform for trading NFL player shares, leveraging real player data from the Sleeper API while maintaining the high-performance characteristics of the core trading system.
