-- Phase 3: Row Level Security (RLS) Policies
-- Locks down tables so frontend direct reads (Phase 4) only expose authorized data.
-- Backend continues using postgres superuser connection which bypasses RLS.

-- ============================================================================
-- 1. User-owned tables (SELECT own data only)
-- ============================================================================

-- accounts: users can only read their own account
ALTER TABLE accounts ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Users can view own account"
  ON accounts FOR SELECT
  USING (supabase_uid = auth.uid());

-- positions: users can only read their own positions
ALTER TABLE positions ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Users can view own positions"
  ON positions FOR SELECT
  USING (account_id = public.get_my_account_id());

-- trades: users can only read their own trades
ALTER TABLE trades ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Users can view own trades"
  ON trades FOR SELECT
  USING (account_id = public.get_my_account_id());

-- reservations: users can only read their own reservations
ALTER TABLE reservations ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Users can view own reservations"
  ON reservations FOR SELECT
  USING (account_id = public.get_my_account_id());

-- daily_equity_snapshots: users can only read their own snapshots
ALTER TABLE daily_equity_snapshots ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Users can view own equity snapshots"
  ON daily_equity_snapshots FOR SELECT
  USING (account_id = public.get_my_account_id());

-- equity_timeseries: users can only read their own equity data
ALTER TABLE equity_timeseries ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Users can view own equity timeseries"
  ON equity_timeseries FOR SELECT
  USING (account_id = public.get_my_account_id());

-- ============================================================================
-- 2. Public read tables (SELECT for all authenticated users)
-- ============================================================================

-- player_metadata: all authenticated users can read
ALTER TABLE player_metadata ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Authenticated users can view player metadata"
  ON player_metadata FOR SELECT
  TO authenticated
  USING (true);

-- price_history: all authenticated users can read
ALTER TABLE price_history ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Authenticated users can view price history"
  ON price_history FOR SELECT
  TO authenticated
  USING (true);

-- rpe_fair_prices: all authenticated users can read
ALTER TABLE rpe_fair_prices ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Authenticated users can view fair prices"
  ON rpe_fair_prices FOR SELECT
  TO authenticated
  USING (true);

-- projections_season: all authenticated users can read
ALTER TABLE projections_season ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Authenticated users can view projections"
  ON projections_season FOR SELECT
  TO authenticated
  USING (true);

-- player_week_points: all authenticated users can read
ALTER TABLE player_week_points ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Authenticated users can view weekly points"
  ON player_week_points FOR SELECT
  TO authenticated
  USING (true);

-- player_id_mapping: all authenticated users can read
ALTER TABLE player_id_mapping ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Authenticated users can view player mapping"
  ON player_id_mapping FOR SELECT
  TO authenticated
  USING (true);

-- market_state: all authenticated users can read
ALTER TABLE market_state ENABLE ROW LEVEL SECURITY;
CREATE POLICY "Authenticated users can view market state"
  ON market_state FOR SELECT
  TO authenticated
  USING (true);

-- ============================================================================
-- 3. Backend-only tables (no frontend access)
-- RLS enabled with NO policies = blocked for all non-superuser roles
-- ============================================================================

ALTER TABLE house_fills ENABLE ROW LEVEL SECURITY;
-- No SELECT policy = blocked for frontend

ALTER TABLE house_accounts ENABLE ROW LEVEL SECURITY;
-- No SELECT policy = blocked for frontend
