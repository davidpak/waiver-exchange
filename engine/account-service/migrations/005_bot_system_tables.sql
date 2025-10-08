-- Bot System Tables Migration
-- This migration creates the tables needed for the dynamic pricing bot system
-- Includes: projections, player data, RPE prices, house fills, player mapping, house accounts, and market state

-- Season projections from SportsDataIO
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

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS rpe_idx ON rpe_fair_prices (player_id, ts);
CREATE INDEX IF NOT EXISTS house_fills_player_ts ON house_fills (player_id, ts);
CREATE INDEX IF NOT EXISTS player_mapping_symbol_id ON player_id_mapping (our_symbol_id);
CREATE INDEX IF NOT EXISTS projections_season_idx ON projections_season (season, fantasy_pos);
CREATE INDEX IF NOT EXISTS player_week_points_idx ON player_week_points (player_id, season, week, ts);
CREATE INDEX IF NOT EXISTS market_state_idx ON market_state (state, last_updated);

-- Add comments for documentation
COMMENT ON TABLE projections_season IS 'Season projections from SportsDataIO for price discovery';
COMMENT ON TABLE player_week_points IS 'Live weekly fantasy points snapshots during games';
COMMENT ON TABLE rpe_fair_prices IS 'Reference Price Engine fair price history';
COMMENT ON TABLE house_fills IS 'House bot fill audit trail for transparency';
COMMENT ON TABLE player_id_mapping IS 'Maps SportsDataIO player IDs to our internal symbol IDs';
COMMENT ON TABLE house_accounts IS 'Special bot accounts for House market making';
COMMENT ON TABLE market_state IS 'Tracks market state per symbol (open/closed/halted/circuit_breaker)';

COMMENT ON COLUMN projections_season.proj_points IS 'Fantasy points projection for the season';
COMMENT ON COLUMN projections_season.fantasy_pos IS 'Fantasy position (QB/RB/WR/TE)';
COMMENT ON COLUMN projections_season.adp IS 'Average Draft Position';
COMMENT ON COLUMN player_week_points.fantasy_pts IS 'Current fantasy points for the week';
COMMENT ON COLUMN player_week_points.is_game_over IS 'Whether the game is finished';
COMMENT ON COLUMN rpe_fair_prices.fair_cents IS 'Fair price in cents (Fₜ)';
COMMENT ON COLUMN rpe_fair_prices.band_bps IS 'Price band in basis points (e.g., 3000 = ±30%)';
COMMENT ON COLUMN rpe_fair_prices.kappa_cents_per_pt IS 'Price sensitivity in cents per fantasy point';
COMMENT ON COLUMN rpe_fair_prices.pacing_mode IS 'Pacing mode: step or poll-step';
COMMENT ON COLUMN rpe_fair_prices.actual_pts IS 'Actual fantasy points at this timestamp';
COMMENT ON COLUMN rpe_fair_prices.delta_pts IS 'Change in fantasy points from previous update';
COMMENT ON COLUMN rpe_fair_prices.reason IS 'Reason for price update (projection, fp_delta, etc.)';
COMMENT ON COLUMN house_fills.order_id IS 'Order ID that was filled by House bot';
COMMENT ON COLUMN house_fills.account_id IS 'Human account that received the fill';
COMMENT ON COLUMN house_fills.player_id IS 'Player/symbol that was traded';
COMMENT ON COLUMN house_fills.price_cents IS 'Price at which House bot filled the order';
COMMENT ON COLUMN house_fills.qty_bp IS 'Quantity filled in basis points';
COMMENT ON COLUMN house_fills.ft_at_fill IS 'Fair price at time of fill';
COMMENT ON COLUMN house_fills.rule_json IS 'House bot rule parameters used for this fill';
COMMENT ON COLUMN player_id_mapping.sportsdataio_player_id IS 'Player ID from SportsDataIO API';
COMMENT ON COLUMN player_id_mapping.our_symbol_id IS 'Our internal symbol ID for trading';
COMMENT ON COLUMN house_accounts.account_type IS 'Type of House account (market_maker, liquidity)';
COMMENT ON COLUMN house_accounts.currency_balance IS 'Available balance for House bot trading';
COMMENT ON COLUMN market_state.state IS 'Current market state (open/closed/halted/circuit_breaker)';
COMMENT ON COLUMN market_state.reason IS 'Reason for current state (e.g., high_volatility)';
