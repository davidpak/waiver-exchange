-- Daily equity snapshots for performance tracking
-- This migration creates a table to store daily equity snapshots for all accounts

CREATE TABLE daily_equity_snapshots (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT NOT NULL REFERENCES accounts(id),
    date DATE NOT NULL,
    total_equity BIGINT NOT NULL, -- In cents
    cash_balance BIGINT NOT NULL, -- In cents
    position_value BIGINT NOT NULL, -- In cents
    day_change BIGINT NOT NULL, -- In cents (vs previous day)
    day_change_percent DECIMAL(10,4) NOT NULL, -- Percentage change
    created_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(account_id, date)
);

-- Indexes for performance
CREATE INDEX idx_daily_equity_account_date ON daily_equity_snapshots(account_id, date);
CREATE INDEX idx_daily_equity_date ON daily_equity_snapshots(date);
