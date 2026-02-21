-- Phase 5: Enable Supabase Realtime on selected tables
-- These tables will push changes to connected clients via WebSocket

-- Enable realtime for fair prices (market data updates)
ALTER PUBLICATION supabase_realtime ADD TABLE rpe_fair_prices;

-- Enable realtime for equity timeseries (user equity updates)
ALTER PUBLICATION supabase_realtime ADD TABLE equity_timeseries;

-- Enable realtime for market state (open/closed/halted)
ALTER PUBLICATION supabase_realtime ADD TABLE market_state;

-- Enable realtime for price history (trade-based candle updates)
ALTER PUBLICATION supabase_realtime ADD TABLE price_history;
