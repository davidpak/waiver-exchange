-- Migration: 002_add_price_history.sql
-- This migration creates the price_history table for storing OHLC candle data
-- Created: 2024-01-15
-- Purpose: Store historical price data for candlestick charts

-- Price history table for OHLC candle data
CREATE TABLE price_history (
    symbol_id INTEGER NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    open_price BIGINT NOT NULL,    -- Price in cents
    high_price BIGINT NOT NULL,    -- Price in cents
    low_price BIGINT NOT NULL,     -- Price in cents
    close_price BIGINT NOT NULL,   -- Price in cents
    volume BIGINT NOT NULL,        -- Number of shares
    PRIMARY KEY (symbol_id, timestamp)
);

-- Indexes for fast lookups
CREATE INDEX idx_price_history_symbol_time ON price_history(symbol_id, timestamp);
CREATE INDEX idx_price_history_timestamp ON price_history(timestamp);
CREATE INDEX idx_price_history_symbol ON price_history(symbol_id);

-- Add comments for documentation
COMMENT ON TABLE price_history IS 'Historical OHLC price data for candlestick charts';
COMMENT ON COLUMN price_history.symbol_id IS 'Symbol ID (matches player symbol_id)';
COMMENT ON COLUMN price_history.timestamp IS 'Candle timestamp (rounded to interval)';
COMMENT ON COLUMN price_history.open_price IS 'Opening price in cents';
COMMENT ON COLUMN price_history.high_price IS 'Highest price in cents';
COMMENT ON COLUMN price_history.low_price IS 'Lowest price in cents';
COMMENT ON COLUMN price_history.close_price IS 'Closing price in cents';
COMMENT ON COLUMN price_history.volume IS 'Total volume (number of shares)';
