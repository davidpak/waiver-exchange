-- Add realized_pnl columns to existing tables for EVS
-- This migration adds realized P&L tracking to accounts and positions tables

-- Add realized_pnl column to accounts table
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS realized_pnl BIGINT DEFAULT 0;

-- Add realized_pnl column to positions table  
ALTER TABLE positions ADD COLUMN IF NOT EXISTS realized_pnl BIGINT DEFAULT 0;

-- Create equity_timeseries table for intraday equity tracking
CREATE TABLE IF NOT EXISTS equity_timeseries (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    tick BIGINT NOT NULL,
    total_equity BIGINT NOT NULL,      -- Total portfolio value in cents
    cash_balance BIGINT NOT NULL,      -- Available cash in cents
    position_value BIGINT NOT NULL,    -- Value of all positions in cents
    unrealized_pnl BIGINT NOT NULL,    -- Unrealized P&L in cents
    realized_pnl BIGINT NOT NULL,      -- Realized P&L in cents
    day_change BIGINT NOT NULL,        -- $ change today in cents
    day_change_percent DECIMAL(10,4) NOT NULL, -- % change today
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_equity_account_timestamp ON equity_timeseries (account_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_equity_timestamp ON equity_timeseries (timestamp);
CREATE INDEX IF NOT EXISTS idx_equity_account_tick ON equity_timeseries (account_id, tick);

-- Add comments for documentation
COMMENT ON COLUMN accounts.realized_pnl IS 'Total realized P&L for this account in cents';
COMMENT ON COLUMN positions.realized_pnl IS 'Realized P&L for this position in cents';
COMMENT ON TABLE equity_timeseries IS 'Intraday equity snapshots for real-time tracking';
COMMENT ON COLUMN equity_timeseries.total_equity IS 'Total portfolio value in cents';
COMMENT ON COLUMN equity_timeseries.cash_balance IS 'Available cash in cents';
COMMENT ON COLUMN equity_timeseries.position_value IS 'Value of all positions in cents';
COMMENT ON COLUMN equity_timeseries.unrealized_pnl IS 'Unrealized P&L in cents (paper gains/losses)';
COMMENT ON COLUMN equity_timeseries.realized_pnl IS 'Realized P&L in cents (actual trading profits/losses)';
COMMENT ON COLUMN equity_timeseries.day_change IS 'Dollar change today in cents';
COMMENT ON COLUMN equity_timeseries.day_change_percent IS 'Percentage change today';
