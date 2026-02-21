-- Combined schema migration for Supabase
-- Run this in Supabase SQL Editor to set up the complete database schema
-- Combines all 7 original migration files in dependency order

-- ============================================================================
-- 1. Core account information (from 001_initial_schema.sql)
-- ============================================================================

CREATE TABLE accounts (
    id BIGSERIAL PRIMARY KEY,
    google_id VARCHAR(255) UNIQUE NOT NULL,
    sleeper_user_id VARCHAR(255),
    sleeper_roster_id VARCHAR(255),
    sleeper_league_id VARCHAR(255),
    display_name VARCHAR(255),
    fantasy_points INTEGER DEFAULT 0,
    weekly_wins INTEGER DEFAULT 0,
    currency_balance BIGINT DEFAULT 0,
    realized_pnl BIGINT DEFAULT 0,
    created_at TIMESTAMP DEFAULT NOW(),
    last_updated TIMESTAMP DEFAULT NOW()
);

CREATE TABLE positions (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT REFERENCES accounts(id),
    symbol_id BIGINT NOT NULL,
    quantity BIGINT NOT NULL,
    avg_cost BIGINT NOT NULL,
    realized_pnl BIGINT DEFAULT 0,
    last_updated TIMESTAMP DEFAULT NOW(),
    UNIQUE(account_id, symbol_id)
);

CREATE TABLE trades (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT REFERENCES accounts(id),
    symbol_id BIGINT NOT NULL,
    side VARCHAR(4) NOT NULL,
    quantity BIGINT NOT NULL,
    price BIGINT NOT NULL,
    timestamp TIMESTAMP DEFAULT NOW(),
    order_id BIGINT NOT NULL
);

CREATE TABLE reservations (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT REFERENCES accounts(id),
    amount BIGINT NOT NULL,
    order_id BIGINT NOT NULL,
    status VARCHAR(20) DEFAULT 'active',
    created_at TIMESTAMP DEFAULT NOW(),
    expires_at TIMESTAMP NOT NULL
);

CREATE INDEX idx_accounts_google_id ON accounts(google_id);
CREATE INDEX idx_accounts_sleeper_user_id ON accounts(sleeper_user_id);
CREATE INDEX idx_accounts_sleeper_league_id ON accounts(sleeper_league_id);
CREATE INDEX idx_positions_account_id ON positions(account_id);
CREATE INDEX idx_trades_account_id ON trades(account_id);
CREATE INDEX idx_reservations_account_id ON reservations(account_id);
CREATE INDEX idx_reservations_expires_at ON reservations(expires_at);

-- ============================================================================
-- 2. Price history (from 002_add_price_history.sql)
-- ============================================================================

CREATE TABLE price_history (
    symbol_id INTEGER NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    open_price BIGINT NOT NULL,
    high_price BIGINT NOT NULL,
    low_price BIGINT NOT NULL,
    close_price BIGINT NOT NULL,
    volume BIGINT NOT NULL,
    PRIMARY KEY (symbol_id, timestamp)
);

CREATE INDEX idx_price_history_symbol_time ON price_history(symbol_id, timestamp);
CREATE INDEX idx_price_history_timestamp ON price_history(timestamp);
CREATE INDEX idx_price_history_symbol ON price_history(symbol_id);

-- ============================================================================
-- 3. Player metadata (from 003_add_player_metadata.sql)
-- ============================================================================

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

CREATE INDEX idx_player_metadata_symbol_id ON player_metadata(symbol_id);
CREATE INDEX idx_player_metadata_name ON player_metadata(name);
CREATE INDEX idx_player_metadata_position ON player_metadata(position);
CREATE INDEX idx_player_metadata_team ON player_metadata(team);
CREATE INDEX idx_player_metadata_rank ON player_metadata(rank);

-- ============================================================================
-- 4. Daily equity snapshots (from 004_add_daily_equity_snapshots.sql)
-- ============================================================================

CREATE TABLE daily_equity_snapshots (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT NOT NULL REFERENCES accounts(id),
    date DATE NOT NULL,
    total_equity BIGINT NOT NULL,
    cash_balance BIGINT NOT NULL,
    position_value BIGINT NOT NULL,
    day_change BIGINT NOT NULL,
    day_change_percent DECIMAL(10,4) NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(account_id, date)
);

CREATE INDEX idx_daily_equity_account_date ON daily_equity_snapshots(account_id, date);
CREATE INDEX idx_daily_equity_date ON daily_equity_snapshots(date);

-- ============================================================================
-- 5. Equity timeseries (from equity-service 001_add_realized_pnl_columns.sql)
-- ============================================================================

CREATE TABLE equity_timeseries (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    tick BIGINT NOT NULL,
    total_equity BIGINT NOT NULL,
    cash_balance BIGINT NOT NULL,
    position_value BIGINT NOT NULL,
    unrealized_pnl BIGINT NOT NULL,
    realized_pnl BIGINT NOT NULL,
    day_change BIGINT NOT NULL,
    day_change_percent DECIMAL(10,4) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_equity_account_timestamp ON equity_timeseries (account_id, timestamp);
CREATE INDEX idx_equity_timestamp ON equity_timeseries (timestamp);
CREATE INDEX idx_equity_account_tick ON equity_timeseries (account_id, tick);

-- ============================================================================
-- 6. Bot system tables (from 005_bot_system_tables.sql)
-- ============================================================================

CREATE TABLE projections_season (
  player_id    INT NOT NULL,
  season       INT NOT NULL,
  proj_points  NUMERIC(6,2) NOT NULL,
  fantasy_pos  TEXT NOT NULL,
  adp          NUMERIC(8,2),
  source       TEXT NOT NULL DEFAULT 'sportsdataio',
  ingested_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (player_id, season)
);

CREATE TABLE player_week_points (
  player_id    INT NOT NULL,
  season       INT NOT NULL,
  week         INT NOT NULL,
  ts           TIMESTAMPTZ NOT NULL,
  fantasy_pts  NUMERIC(6,2) NOT NULL,
  is_game_over BOOLEAN,
  raw          JSONB NOT NULL,
  PRIMARY KEY (player_id, season, week, ts)
);

CREATE TABLE rpe_fair_prices (
  player_id           INT NOT NULL PRIMARY KEY,
  ts                  TIMESTAMPTZ NOT NULL,
  season              INT NOT NULL,
  week                INT,
  fair_cents          BIGINT NOT NULL,
  band_bps            INT NOT NULL,
  kappa_cents_per_pt  INT NOT NULL,
  pacing_mode         TEXT NOT NULL,
  actual_pts          NUMERIC(6,2) NOT NULL,
  delta_pts           NUMERIC(6,2) NOT NULL,
  reason              JSONB NOT NULL,
  source              TEXT DEFAULT 'projection',
  confidence_score    DECIMAL(3,2) DEFAULT 0.5
);

CREATE TABLE house_fills (
  id BIGSERIAL PRIMARY KEY,
  order_id     BIGINT NOT NULL,
  account_id   BIGINT NOT NULL,
  player_id    INT NOT NULL,
  ts           TIMESTAMPTZ NOT NULL,
  price_cents  BIGINT NOT NULL,
  qty_bp       BIGINT NOT NULL,
  ft_at_fill   BIGINT NOT NULL,
  rule_json    JSONB NOT NULL
);

CREATE TABLE player_id_mapping (
  sportsdataio_player_id INT NOT NULL,
  our_symbol_id          INT NOT NULL,
  player_name            TEXT NOT NULL,
  team                   TEXT,
  position               TEXT,
  created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (sportsdataio_player_id),
  UNIQUE(our_symbol_id)
);

CREATE TABLE house_accounts (
  id BIGSERIAL PRIMARY KEY,
  account_type TEXT NOT NULL,
  display_name TEXT NOT NULL,
  currency_balance BIGINT DEFAULT 0,
  is_active BOOLEAN DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE market_state (
  symbol_id INT NOT NULL,
  state TEXT NOT NULL,
  reason TEXT,
  last_updated TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (symbol_id)
);

CREATE INDEX rpe_idx ON rpe_fair_prices (player_id, ts);
CREATE INDEX house_fills_player_ts ON house_fills (player_id, ts);
CREATE INDEX player_mapping_symbol_id ON player_id_mapping (our_symbol_id);
CREATE INDEX projections_season_idx ON projections_season (season, fantasy_pos);
CREATE INDEX player_week_points_idx ON player_week_points (player_id, season, week, ts);
CREATE INDEX market_state_idx ON market_state (state, last_updated);
