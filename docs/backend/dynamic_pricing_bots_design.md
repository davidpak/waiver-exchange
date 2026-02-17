> **Implementation Status:** RPE (Fair Price 2.3) is fully implemented. Market maker service exists and connects via WebSocket. Router-level micro-auction batching and House fallback logic described here are not yet implemented. SportsDataIO integration is working with free-tier API limits.

---

# Bot Integration and Reference Price Engine Doc
This is the single source of truth for implementing a lively, fair, and efficient fantasy-player exchange with minimal liquidity and strict API limits.

- **Data:** SportsDataIO via exactly **two endpoints** (Season Projections; Player Game Stats by Week).
- **Pricing:** Reference Price Engine (RPE) drives **F₀/Fₜ** from projections + live fantasy points.
- **Matching:** Keep **Whistle** unchanged; add a thin **Router** in front to handle micro-auction batching, admission/collars, and optionally House fallback (no resting bot quotes).
- **Fairness:** Humans match humans first. House fills residuals only if strict gates pass; “ridiculous” orders are rejected or expire.
- **Budget:** Designed to fit under **1,000 calls/month** (free tier) with 10-minute polling cadence.

---

## 0) Goals & Non-Goals

- **Goals:** Real-time-feeling charts, instant usability (first user can trade), minimal bot dominance, strong fairness/abuse controls, low ops cost.
- **Non-Goals:** Building a full market-maker bot, per-second play-by-play, or visible resting bot quotes (v1).

---

## 1) High-Level Architecture

```
SportsDataIO (2 endpoints)
      │
      ▼
Fetcher (nightly projections; game-window weekly stats @ 10m)
      │  emits PlayerWeekPointsUpdate
      ▼
RPE (F₀/Fₜ from projections + fantasy points deltas; optional poll-step pacing)
      │  emits RpeFairPriceUpdate
      ├───────────────► Gateway WS (coalesce; 250–500ms tick; UI charts/P&L)
      │
      ▼
Router (admission + micro-auction batch + House fallback IOC)
      │  submits/cancels via existing client
      ▼
Whistle (unchanged price-time matcher)
      │  trade/settlement events
      ▼
ExecutionManager → EVS (mark at Fₜ; equity_timeseries)

```

- **No changes** inside Whistle or EVS interfaces.

---

## 2) External Data (only two endpoints)

**(A) Season Projections**

`NFL/Projections/PlayerSeasonProjectionStats/{season}`

HTTP endpoint: `https://api.sportsdata.io/v3/nfl/projections/json/PlayerSeasonProjectionStats/{season}?key=2d60a5317f014813810755b281f8c2ea`

Key: `2d60a5317f014813810755b281f8c2ea`

Use: `PlayerID`, `FantasyPoints` or `FantasyPointsPPR`, `FantasyPosition`, optional `AverageDraftPosition`.

Cadence: **once nightly**.

**(B) Player Game Stats by Week (Live & Final)**

`NFL/Stats/PlayerGameStatsByWeek/{season}/{week}`

Key: `6a0d677700b24336990b4525be87ca82`

Use: `PlayerID`, `FantasyPoints` (or PPR), `IsGameOver`.

Cadence: **only during live windows** (Thu/Sun/Mon). **10-minute** polling on free tier.

**Server-side only.** One poll returns all players for the week. Don’t embed keys in clients.

---

## 3) API Budgeting

- Projections: ~30 calls/month.
- Weekly stats (10-minute cadence across TNF/SUN/MNF windows): ~384 calls/month.
- Total ~414 calls/month → **well under 1,000**.
    
    Guardrails: poll only in windows; cache & skip unchanged payloads; single poller service.
    

---

## 4) Data Model (DB)

```sql
-- Season projections
CREATE TABLE IF NOT EXISTS projections_season (
  player_id    INT NOT NULL,
  season       INT NOT NULL,
  proj_points  NUMERIC(6,2) NOT NULL,   -- FP or FP_PPR
  fantasy_pos  TEXT NOT NULL,           -- QB/RB/WR/TE
  adp          NUMERIC(8,2),
  source       TEXT NOT NULL DEFAULT 'sportsdataio',
  ingested_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (player_id, season)
);

-- Live weekly fantasy points snapshots (append-only)
CREATE TABLE IF NOT EXISTS player_week_points (
  player_id    INT NOT NULL,
  season       INT NOT NULL,
  week         INT NOT NULL,
  ts           TIMESTAMPTZ NOT NULL,
  fantasy_pts  NUMERIC(6,2) NOT NULL,
  is_game_over BOOLEAN,
  raw          JSONB NOT NULL,
  PRIMARY KEY (player_id, season, week, ts)
);

-- Reference price history (append-only)
CREATE TABLE IF NOT EXISTS rpe_fair_prices (
  player_id           INT NOT NULL,
  ts                  TIMESTAMPTZ NOT NULL,
  season              INT NOT NULL,
  week                INT,
  fair_cents          BIGINT NOT NULL,  -- Fₜ
  band_bps            INT NOT NULL,     -- e.g., 3000 = ±30%
  kappa_cents_per_pt  INT NOT NULL,     -- κ
  pacing_mode         TEXT NOT NULL,    -- 'step' | 'poll-step'
  actual_pts          NUMERIC(6,2) NOT NULL,
  delta_pts           NUMERIC(6,2) NOT NULL,
  reason              JSONB NOT NULL,   -- {"projection":true} or {"fp_delta":+6.0}
  PRIMARY KEY (player_id, ts)
);
CREATE INDEX IF NOT EXISTS rpe_idx ON rpe_fair_prices (player_id, ts);

-- House audit (fills only; no resting quotes in v1)
CREATE TABLE IF NOT EXISTS house_fills (
  id BIGSERIAL PRIMARY KEY,
  order_id     BIGINT NOT NULL,
  account_id   BIGINT NOT NULL,         -- human account
  player_id    INT NOT NULL,
  ts           TIMESTAMPTZ NOT NULL,
  price_cents  BIGINT NOT NULL,
  qty_bp       BIGINT NOT NULL,
  ft_at_fill   BIGINT NOT NULL,
  rule_json    JSONB NOT NULL           -- params used (λ, skew, band, exposure, etc.)
);
CREATE INDEX IF NOT EXISTS house_fills_player_ts ON house_fills (player_id, ts);

-- Player ID mapping (SportsDataIO → our symbol IDs)
CREATE TABLE IF NOT EXISTS player_id_mapping (
  sportsdataio_player_id INT NOT NULL,
  our_symbol_id          INT NOT NULL,
  player_name            TEXT NOT NULL,
  team                   TEXT,
  position               TEXT,
  created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (sportsdataio_player_id),
  UNIQUE(our_symbol_id)
);
CREATE INDEX IF NOT EXISTS player_mapping_symbol_id ON player_id_mapping (our_symbol_id);

-- House accounts (special bot accounts)
CREATE TABLE IF NOT EXISTS house_accounts (
  id BIGSERIAL PRIMARY KEY,
  account_type TEXT NOT NULL, -- 'house_market_maker', 'house_liquidity'
  display_name TEXT NOT NULL,
  currency_balance BIGINT DEFAULT 0,
  is_active BOOLEAN DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Market state tracking
CREATE TABLE IF NOT EXISTS market_state (
  symbol_id INT NOT NULL,
  state TEXT NOT NULL, -- 'open', 'closed', 'halted', 'circuit_breaker'
  reason TEXT,
  last_updated TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (symbol_id)
);
```

*(Use your existing `equity_timeseries` for EVS snapshots.)*

---

## 5) Reference Price Engine (RPE)

**Pre-game F₀ (seed):**

```
P_season = FantasyPoints (or PPR)
P_week   = P_season / 17

F₀ = base
   + β_season * (P_season / 17)
   + β_week   * P_week

```

Starter knobs: `base=$50`, `β_season=$2`, `β_week=$8`.

κ (sensitivity $/pt): WR/RB/TE=$1.5, QB=$1.0.

Band: ±30–40% intragame clamp around baseline.

**In-game Fₜ (no clock; live deltas only):**

```
Δpts = FantasyPoints_now − FantasyPoints_prev
Fₜ  = clip(F_(t−1) + κ * Δpts, band)

```

Optional **poll-step pacing** between polls (no extra endpoints): define N steps per game (6 at 10-min cadence); drift 10–15% per WS tick toward

```
F_drift_target = F₀ + κ * (FantasyPoints_now − P_week * step/N)

```

Emit `RpeFairPriceUpdate` on change ≥ 1 tick.

---

## 6) Fetcher

- **Nightly:** projections → upsert `projections_season`.
- **Game windows (Thu/Sun/Mon):** poll `PlayerGameStatsByWeek` every 10 min → append `player_week_points` (dedupe by hash) → emit `PlayerWeekPointsUpdate`.
- Backoff on 429/5xx; surface a “data delayed” flag to UI if needed.

---

## 7) Router (enhanced with fair price integration)

**Responsibilities**

1. **Admission checks** (reject before Whistle).
2. **Fair price integration** for price collar validation.
3. **Basic liquidity provision** via simple market making.

**Admission checks (hard rules):**

- **Price collar**: reject if limit outside ±`price_collar_bps` of Fₜ (e.g., ±5%).
- **Max notional** per order; **min qty** to avoid dust.
- **Throttle** per account/symbol (e.g., ≤20 orders/10s).
- **State**: symbol tradable & within mode (e.g., breaker may force PO).
- **Order types**: allow LIMIT, IOC, POST_ONLY (no true MARKET at v1).

**Fair price integration:**

- Lookup Fₜ from RPE engine for price collar validation.
- Cache fair prices for performance (update every 1-5 seconds).
- Reject orders outside price collar before sending to Whistle.

**Basic market making (liquidity provision):**

- Simple market maker posts bid/ask quotes around Fₜ.
- **Spread**: ±`market_maker_spread_bps` of Fₜ (e.g., ±3-5%).
- **Update frequency**: Every 10-30 seconds or on significant Fₜ changes.
- **Order management**: Cancel old quotes before posting new ones.
- **Risk limits**: Max position per player, daily notional caps.

**Simplified approach** → minimal complexity, leverages existing order flow.

---

## 8) Market Maker Service (simple liquidity provision)

**Purpose**: Provide basic liquidity by posting bid/ask quotes around fair prices.

**Configuration**:
- **Spread**: ±3-5% of Fₜ (configurable per player/position)
- **Update frequency**: 10-30 seconds or on Fₜ changes >1%
- **Position limits**: Max shares per player, daily notional caps
- **Risk management**: Stop trading if position exceeds limits

**Operation**:
1. **Quote posting**: Post BUY @ (Fₜ - spread) and SELL @ (Fₜ + spread)
2. **Order management**: Cancel old quotes before posting new ones
3. **Fair price updates**: Adjust quotes when Fₜ changes significantly
4. **Risk monitoring**: Monitor position and stop if limits exceeded

**Integration**:
- Uses existing OrderGateway/OrderRouter flow
- No changes to Whistle or ExecutionManager
- Leverages RPE engine for fair price updates

---

## 9) EVS (Equity Valuation Service)

- Mark = **Fₜ**; update all holders when they trade or their symbols’ **Fₜ** changed.
- Integer math:
    - `value_cents = (qty_bp * fair_cents) / 10_000`
    - `unrealized = (qty_bp * (fair − avg_cost)) / 10_000`
- Persist `equity_timeseries`; maintain in-memory `symbol→holders` index.

---

## 10) Circuit Breakers, Cooldowns, Abuse Guards

**Triggers** (per symbol):

- |Fₜ − F₀| > `breaker_band_bps` (e.g., 40%).
- High volatility: >N RPE updates in M seconds.
- Bot share: `house_usage_ratio` > 60% for rolling 5–15 min.

**Actions:**

- Temporarily widen collars (e.g., to ±8–10%).
- Force **POST_ONLY** (no takers) for X minutes.
- Disable House for X minutes.
- UI banner “Volatility controls active.”

**Abuse guards:**

- Rate-limit “ping-pong” (rapid buy/sell alternations).
- Spoof/cancel rate thresholds → PO-only for the account.
- Wash-trade guard: reject self/linked account crosses.

---

## 11) Configuration (single source of truth)

```yaml
sportsdataio:
  api_key_env: SPORTS_DATA_IO_KEY
  season: "2025REG"
  week: 4
  poll_minutes: 10
  live_windows_utc:
    - { day: "THU", start: "00:00", end: "08:00" }
    - { day: "SUN", start: "17:00", end: "02:00+1" }
    - { day: "MON", start: "00:00", end: "06:00" }

rpe:
  base_cents: 5000
  beta_season_cents_per_pt: 200
  beta_week_cents_per_pt: 800
  kappa_cents_per_pt: { default: 150, QB: 100 }
  ingame_band_bps: 3000
  pacing_mode: step          # 'step' or 'poll-step'
  pacing_steps: 6            # 10m cadence → ~6 steps/game

router:
  price_collar_bps: 500          # ±5% of Fₜ
  min_qty_bp: 10000             # 1 share minimum
  max_order_notional_cents: 250000  # $2,500 max per order
  throttle: { max_orders_per_10s: 20 }
  fair_price_cache_seconds: 5   # Cache fair prices for 5 seconds
  order_types: [LIMIT, IOC, POST_ONLY]

market_maker:
  enabled: true
  spread_bps: 400               # ±4% spread around Fₜ
  update_frequency_seconds: 15  # Update quotes every 15 seconds
  max_position_per_player: 100  # Max 100 shares per player
  daily_notional_limit_cents: 10000000  # $100k daily limit
  risk_check_interval_seconds: 30

circuit_breakers:
  breaker_band_bps: 4000
  max_updates_in_window: { count: 12, seconds: 120 }
  actions:
    widen_collar_bps: 1000
    force_post_only: true
    disable_market_maker_minutes: 10

# Integration with existing system
integration:
  # Use existing WebSocket API
  websocket_endpoint: "ws://localhost:8080/ws"
  
  # Use existing OrderGateway for bot orders
  order_gateway_url: "http://localhost:8080"
  
  # Use existing EVS for price marking
  evs_integration: true
  
  # Use existing database
  database_url: "postgresql://user:pass@localhost/waiver_exchange"
  
  # Player ID mapping strategy
  player_mapping:
    strategy: "database_table"  # or "json_file" or "api_lookup"
    fallback_to_default: true
    default_price_cents: 5000

```

---

## 11) Metrics & Alerts

- **Fetcher:** `sio_requests{endpoint,status}`, `429_backoffs_total`, `payload_unchanged_total`.
- **RPE:** `rpe_updates_total`, `rpe_fp_delta_sum`, `rpe_latency_ms`.
- **Router:** `micro_auction_utilization`, `collar_rejections_total`, `order_reject_reasons{}`.
- **House:** `house_usage_ratio`, `house_notional{player}`, `avg_slippage_bps`.
- **EVS:** `holders_updated_total`, `equity_writes_total`, `tick_latency_ms`.
- **Gateway:** `ws_connected`, `ws_msgs_sent_total`.

**Alerts:** 429 streaks, House usage >60% (15m), EVS lag >2 ticks, high reject rates.

---

## 12) Testing

**Unit**

- RPE: `Δfp=+6.0` → `ΔF ≈ κ*6` within band.
- Router: collar acceptance/rejection; batch timer behavior.
- House: eligibility gates; price calc; cap enforcement.

**Integration**

- Day-0 boot: projections → F₀ broadcast for all players.
- Human↔human: two opposing orders within 300 ms cross; no House rows.
- Residual: single order → partial human fill + House IOC for remainder.
- Collar: absurd price rejected; no Whistle submission.
- Breaker: trigger → POST_ONLY enforced; House disabled.

**E2E Budget**

- Simulate one month of windows @ 10-min cadence → total API calls < 900.

---

## 13) Rollout (safe toggles)

1. Deploy Fetcher (projections nightly; stats in windows).
2. Deploy RPE (“step” mode).
3. Deploy Router **with collars only** (pass-through).
4. Enable micro-auction (300–500 ms).
5. Enable House with tiny caps; monitor metrics.
6. Tune λ, band; enable “poll-step” pacing if desired.
7. Turn on alerts; review `house_fills` audits.

---

## 14) Frontend Notes (Mantine UI)

- Subscribe to WS fair-price/equity deltas (250–500 ms).
- Animate chart to new **Fₜ** over 300–500 ms per update; keep a subtle idle shimmer between polls.
- Display day change/percent using EVS snapshots; avoid big spacing/gaps (tighten `gap="xs"` and ensure no unintended margins on nested Groups/Stacks).
- Show banners for “Volatility controls active” and “Data delayed” flags.

---

## 15) Security & Keys

- Keys only on server; single poller; respect rate limits.
- Do not expose SportsDataIO responses to clients; we fan out our own normalized events.

---

### Appendix A — House Price Rule (Deterministic)

```
input: side ∈ {BUY, SELL}, fair Fₜ, λ (ticks), skew (±≤1 tick), fill_band_bps
candidate = Fₜ ± ticks(λ) ± ticks(skew)   // + for BUY, - for SELL from the user’s POV
price = clip(candidate, Fₜ ± fill_band)

```

Record rule inputs in `house_fills.rule_json` (λ, skew, band, exposure before/after, i_t).

---

### Appendix B — Router Pseudocode (IOC fallback)
```
fn admit(o, F) -> Result<()> {
  if !within_collar(o.limit_px, F, collar_bps) { bail!("collar"); }
  if notional(o) > max_notional { bail!("size"); }
  if o.qty_bp < min_qty_bp { bail!("dust"); }
  if throttled(o.account) { bail!("rate"); }
  Ok(())
}

async fn on_order(o) {
  let F = fair(o.player);
  admit(o, F)?;
  stage_in_batch(o.player, o);                 // 300–500 ms queue
}

async fn on_batch_close(player, batch) {
  submit_all_to_whistle(batch);                // unchanged API
  wait_fills_80ms();
  let residuals = compute_unfilled(batch);
  if residuals.is_empty() || !cfg.house.enabled { return; }

  for r in residuals {
    if !house_eligible(player, r) { continue; }
    let px = house_price(F, r.side, cfg.house, short_term_imbalance(player));
    let ioc = mk_ioc_counter(r, px, cfg.house.fill_band_bps);
    whistle.submit(ioc).await?;                // any remainder cancels
    audit_house_intent(ioc, F, cfg.house);
  }
}
```

---

## 16) Integration with Existing System

### 16.1 Player ID Mapping Strategy

**Problem**: SportsDataIO uses different player IDs than our internal symbol IDs.

**Solution**: Database table mapping with fallback logic:

```sql
-- Insert mapping data during initial setup
INSERT INTO player_id_mapping (sportsdataio_player_id, our_symbol_id, player_name, team, position)
VALUES 
  (12345, 1, 'Josh Allen', 'BUF', 'QB'),
  (67890, 2, 'Christian McCaffrey', 'SF', 'RB'),
  -- ... etc
```

**Fallback Logic**:
1. Look up in `player_id_mapping` table
2. If not found, use default price from config
3. Log unmapped players for manual review

### 16.2 House Account Management

**Problem**: Need special bot accounts that don't use Google OAuth.

**Solution**: Create dedicated House accounts:

```sql
-- Create House accounts during setup
INSERT INTO house_accounts (account_type, display_name, currency_balance)
VALUES 
  ('house_market_maker', 'House Market Maker', 100000000), -- $1M
  ('house_liquidity', 'House Liquidity', 500000000);        -- $5M
```

**Authentication**: Use API key authentication for House accounts, bypassing OAuth.

### 16.3 WebSocket API Extensions

**New Events**:
```json
// Fair price update
{
  "stream": "rpe_fair_price_update",
  "data": {
    "symbol_id": 1,
    "fair_price_cents": 5500,
    "band_bps": 3000,
    "reason": "fp_delta",
    "timestamp": "2025-01-15T10:30:00Z"
  }
}

// House fill notification
{
  "stream": "house_fill",
  "data": {
    "order_id": "ord_12345",
    "account_id": 7,
    "symbol_id": 1,
    "price_cents": 5450,
    "quantity": 1000,
    "timestamp": "2025-01-15T10:30:00Z"
  }
}

// Circuit breaker status
{
  "stream": "market_state",
  "data": {
    "symbol_id": 1,
    "state": "circuit_breaker",
    "reason": "high_volatility",
    "timestamp": "2025-01-15T10:30:00Z"
  }
}
```

### 16.4 EVS Integration

**Price Marking**: EVS will use RPE fair prices instead of last trade prices:

```rust
// In EVS service
async fn get_current_price(&self, symbol_id: u32) -> Result<i64> {
    // Try RPE fair price first
    if let Some(fair_price) = self.get_rpe_fair_price(symbol_id).await? {
        return Ok(fair_price);
    }
    
    // Fallback to last trade price
    // ... existing logic
}
```

### 16.5 Market State Management

**States**: `open`, `closed`, `halted`, `circuit_breaker`

**Transitions**:
- `open` → `circuit_breaker`: High volatility detected
- `circuit_breaker` → `open`: Cooldown period expired
- `open` → `closed`: Market hours ended
- `closed` → `open`: Market hours started

---

## 17) Implementation Phases

### Phase 1: Foundation (2-3 weeks)
1. **Database schema updates**
   - Add new tables and migrations
   - Create player ID mapping data
   - Set up House accounts

2. **SportsDataIO Fetcher**
   - External data ingestion service
   - Basic error handling and retry logic
   - Configuration management

3. **Basic RPE**
   - Simple price calculation without pacing
   - Database persistence
   - WebSocket event emission

### Phase 2: Core Bot Logic (3-4 weeks)
1. **Enhanced Router**
   - Micro-auction batching
   - Price collar validation
   - House fallback logic

2. **House Bot**
   - Basic liquidity provision
   - IOC order submission
   - Fill auditing

3. **WebSocket extensions**
   - Fair price updates
   - House fill notifications
   - Market state updates

### Phase 3: Production Features (2-3 weeks)
1. **Circuit breakers**
   - Volatility detection
   - Market state management
   - UI notifications

2. **Advanced RPE**
   - Poll-step pacing and drift
   - Position-specific sensitivity
   - Advanced price bands

3. **Comprehensive metrics**
   - Full observability
   - Alerting system
   - Performance monitoring

### Phase 4: Testing & Rollout (1-2 weeks)
1. **Integration testing**
   - End-to-end bot scenarios
   - Circuit breaker testing
   - API failure simulation

2. **Gradual rollout**
   - Feature flags
   - Monitoring setup
   - Performance tuning

---

## 18) Risk Mitigation

### 18.1 External API Dependencies
- **Risk**: SportsDataIO API failures
- **Mitigation**: 
  - Graceful degradation to last known prices
  - Retry logic with exponential backoff
  - "Data delayed" UI notifications

### 18.2 Bot Dominance
- **Risk**: House bot taking over market
- **Mitigation**:
  - Strict usage caps and cooldowns
  - Circuit breakers for high bot activity
  - Transparent fill notifications

### 18.3 Price Manipulation
- **Risk**: Users gaming the system
- **Mitigation**:
  - Price collars and validation
  - Rate limiting and throttling
  - Wash trade detection

### 18.4 System Performance
- **Risk**: Bot activity impacting performance
- **Mitigation**:
  - Tick-bounded execution
  - Async processing for non-critical paths
  - Comprehensive monitoring

---

## 19) Success Metrics

### 19.1 Market Quality
- **Liquidity**: First user can trade within 5 seconds
- **Price Discovery**: Fair prices within 10% of projections
- **Volatility**: Price changes < 20% per hour during normal conditions

### 19.2 System Performance
- **Latency**: Order placement < 200ms end-to-end
- **Throughput**: Handle 1000+ orders/minute
- **Uptime**: 99.9% availability during market hours

### 19.3 User Experience
- **Charts**: Real-time price updates every 250-500ms
- **Trading**: Instant order placement and confirmation
- **Transparency**: Clear indication of bot activity

---

### Appendix C — Integration Checklist

**Database Setup**:
- [ ] Run migrations for new tables
- [ ] Populate player ID mapping
- [ ] Create House accounts
- [ ] Set up indexes

**Service Deployment**:
- [ ] Deploy SportsDataIO Fetcher
- [ ] Deploy RPE service
- [ ] Deploy enhanced Router
- [ ] Deploy House Bot

**Configuration**:
- [ ] Set up YAML config files
- [ ] Configure API keys
- [ ] Set up monitoring
- [ ] Configure alerts

**Testing**:
- [ ] Unit tests for all components
- [ ] Integration tests for bot scenarios
- [ ] End-to-end testing
- [ ] Performance benchmarking

**Rollout**:
- [ ] Feature flags enabled
- [ ] Gradual rollout plan
- [ ] Monitoring dashboard
- [ ] Rollback procedures