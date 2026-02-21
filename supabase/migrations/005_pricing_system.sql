-- Phase 5: Multi-source pricing system
-- New tables for crowd-sourced values, computed fair prices, and admin tooling

-- ============================================================================
-- 1. ALTER accounts: add is_admin flag
-- ============================================================================

ALTER TABLE accounts ADD COLUMN IF NOT EXISTS is_admin BOOLEAN DEFAULT false;

-- ============================================================================
-- 2. source_values — Raw data from external sources (immutable, append-only)
-- ============================================================================

CREATE TABLE source_values (
  id            BIGSERIAL PRIMARY KEY,
  source        TEXT NOT NULL,            -- 'fantasycalc', 'ktc', 'sleeper_proj', 'sleeper_stats'
  season        INT NOT NULL,
  week          INT NOT NULL DEFAULT 0,   -- 0 = preseason/dynasty
  player_name   TEXT NOT NULL,
  position      TEXT,
  team          TEXT,
  raw_value     NUMERIC(12,2) NOT NULL,   -- source-native value (trade value, projected pts, etc.)
  source_player_id TEXT,                  -- ID in the external source system
  fetched_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  meta          JSONB DEFAULT '{}'::jsonb  -- extra source-specific fields
);

CREATE INDEX idx_source_values_source_season ON source_values (source, season, week);
CREATE INDEX idx_source_values_player ON source_values (player_name, source);
CREATE INDEX idx_source_values_fetched ON source_values (fetched_at);

-- ============================================================================
-- 3. fair_prices — New pricing model output
-- ============================================================================

CREATE TABLE fair_prices (
  symbol_id             INT NOT NULL,
  season                INT NOT NULL,
  week                  INT NOT NULL DEFAULT 0,
  fair_price_cents      BIGINT NOT NULL,          -- $0.01 - $200.00 (1 - 20000)
  composite_percentile  NUMERIC(6,4) NOT NULL,    -- 0.0000 - 1.0000
  crowd_percentile      NUMERIC(6,4),
  projection_percentile NUMERIC(6,4),
  performance_percentile NUMERIC(6,4),
  confidence            NUMERIC(4,3) NOT NULL,    -- 0.000 - 1.000
  config_snapshot       JSONB NOT NULL DEFAULT '{}'::jsonb,  -- pricing params used
  calculated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (symbol_id, season, week)
);

CREATE INDEX idx_fair_prices_season_week ON fair_prices (season, week);
CREATE INDEX idx_fair_prices_calculated ON fair_prices (calculated_at);

-- ============================================================================
-- 4. pricing_config — Model parameters
-- ============================================================================

CREATE TABLE pricing_config (
  id            SERIAL PRIMARY KEY,
  label         TEXT NOT NULL UNIQUE,       -- 'default', 'aggressive', etc.
  mu            NUMERIC(6,3) NOT NULL DEFAULT 7.6,
  sigma         NUMERIC(6,3) NOT NULL DEFAULT 1.0,
  gamma         NUMERIC(6,3) NOT NULL DEFAULT 0.65,
  p_max_cents   INT NOT NULL DEFAULT 20000,
  p_min_cents   INT NOT NULL DEFAULT 1,
  crossover_pct NUMERIC(4,3) NOT NULL DEFAULT 0.900,
  crowd_floor   NUMERIC(4,3) NOT NULL DEFAULT 0.500,
  crowd_decay   NUMERIC(4,3) NOT NULL DEFAULT 0.200,
  proj_decay    NUMERIC(4,3) NOT NULL DEFAULT 0.250,
  is_active     BOOLEAN NOT NULL DEFAULT false,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed the default config
INSERT INTO pricing_config (label, is_active) VALUES ('default', true);

-- ============================================================================
-- 5. player_source_mapping — Cross-source player name/ID mapping
-- ============================================================================

CREATE TABLE player_source_mapping (
  id              BIGSERIAL PRIMARY KEY,
  symbol_id       INT NOT NULL,              -- our internal symbol_id
  source          TEXT NOT NULL,             -- 'fantasycalc', 'ktc', 'sleeper'
  source_player_id TEXT,                     -- ID in external source
  source_name     TEXT NOT NULL,             -- name as it appears in that source
  match_score     NUMERIC(4,3),             -- 0.000 - 1.000 confidence of match
  verified        BOOLEAN DEFAULT false,     -- admin-confirmed
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(symbol_id, source)
);

CREATE INDEX idx_player_source_mapping_source ON player_source_mapping (source);
CREATE INDEX idx_player_source_mapping_symbol ON player_source_mapping (symbol_id);

-- ============================================================================
-- 6. admin_actions — Audit trail
-- ============================================================================

CREATE TABLE admin_actions (
  id          BIGSERIAL PRIMARY KEY,
  account_id  BIGINT NOT NULL REFERENCES accounts(id),
  action      TEXT NOT NULL,             -- 'fetch_source', 'calculate_prices', 'update_mapping', etc.
  details     JSONB DEFAULT '{}'::jsonb, -- action-specific metadata
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_admin_actions_account ON admin_actions (account_id);
CREATE INDEX idx_admin_actions_action ON admin_actions (action, created_at);

-- ============================================================================
-- 7. Row Level Security
-- ============================================================================

-- source_values: public SELECT for authenticated, writes via service role only
ALTER TABLE source_values ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Authenticated users can view source values"
  ON source_values FOR SELECT
  TO authenticated
  USING (true);

-- fair_prices: public SELECT for authenticated, writes via service role only
ALTER TABLE fair_prices ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Authenticated users can view fair prices"
  ON fair_prices FOR SELECT
  TO authenticated
  USING (true);

-- pricing_config: public SELECT for authenticated, writes via service role only
ALTER TABLE pricing_config ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Authenticated users can view pricing config"
  ON pricing_config FOR SELECT
  TO authenticated
  USING (true);

-- player_source_mapping: public SELECT for authenticated, writes via service role only
ALTER TABLE player_source_mapping ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Authenticated users can view player source mapping"
  ON player_source_mapping FOR SELECT
  TO authenticated
  USING (true);

-- admin_actions: admin-only SELECT and INSERT
ALTER TABLE admin_actions ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Admins can view admin actions"
  ON admin_actions FOR SELECT
  TO authenticated
  USING (
    EXISTS (
      SELECT 1 FROM accounts
      WHERE accounts.supabase_uid = auth.uid()
        AND accounts.is_admin = true
    )
  );
CREATE POLICY "Admins can insert admin actions"
  ON admin_actions FOR INSERT
  TO authenticated
  WITH CHECK (
    EXISTS (
      SELECT 1 FROM accounts
      WHERE accounts.supabase_uid = auth.uid()
        AND accounts.is_admin = true
    )
  );

-- ============================================================================
-- 8. Enable Realtime on fair_prices
-- ============================================================================

ALTER PUBLICATION supabase_realtime ADD TABLE fair_prices;
