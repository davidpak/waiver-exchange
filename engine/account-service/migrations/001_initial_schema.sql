-- Initial schema for AccountService
-- This migration creates the core tables for account management, positions, trades, and reservations

-- Core account information
CREATE TABLE accounts (
    id BIGSERIAL PRIMARY KEY,
    google_id VARCHAR(255) UNIQUE NOT NULL,
    sleeper_user_id VARCHAR(255), -- Store user_id, not username
    sleeper_roster_id VARCHAR(255), -- Their roster in the chosen league
    sleeper_league_id VARCHAR(255), -- The ONE league they chose
    display_name VARCHAR(255),
    fantasy_points INTEGER DEFAULT 0,
    weekly_wins INTEGER DEFAULT 0, -- Track weekly wins for bonuses
    currency_balance BIGINT DEFAULT 0, -- Stored in cents
    created_at TIMESTAMP DEFAULT NOW(),
    last_updated TIMESTAMP DEFAULT NOW()
);

-- Position tracking per symbol
CREATE TABLE positions (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT REFERENCES accounts(id),
    symbol_id BIGINT NOT NULL,
    quantity BIGINT NOT NULL, -- Stored as basis points (1/10000th of a share)
    avg_cost BIGINT NOT NULL, -- Average cost in cents
    last_updated TIMESTAMP DEFAULT NOW(),
    UNIQUE(account_id, symbol_id)
);

-- Trade history
CREATE TABLE trades (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT REFERENCES accounts(id),
    symbol_id BIGINT NOT NULL,
    side VARCHAR(4) NOT NULL, -- 'BUY' or 'SELL'
    quantity BIGINT NOT NULL,
    price BIGINT NOT NULL, -- Price in cents
    timestamp TIMESTAMP DEFAULT NOW(),
    order_id BIGINT NOT NULL
);

-- Balance reservations for limit orders
CREATE TABLE reservations (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT REFERENCES accounts(id),
    amount BIGINT NOT NULL, -- Amount in cents
    order_id BIGINT NOT NULL,
    status VARCHAR(20) DEFAULT 'active', -- 'active', 'settled', 'expired', 'cancelled'
    created_at TIMESTAMP DEFAULT NOW(),
    expires_at TIMESTAMP NOT NULL
);

-- Indexes for performance
CREATE INDEX idx_accounts_google_id ON accounts(google_id);
CREATE INDEX idx_accounts_sleeper_user_id ON accounts(sleeper_user_id);
CREATE INDEX idx_accounts_sleeper_league_id ON accounts(sleeper_league_id);
CREATE INDEX idx_positions_account_id ON positions(account_id);
CREATE INDEX idx_trades_account_id ON trades(account_id);
CREATE INDEX idx_reservations_account_id ON reservations(account_id);
CREATE INDEX idx_reservations_expires_at ON reservations(expires_at);
