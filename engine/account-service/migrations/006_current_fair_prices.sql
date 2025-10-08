-- Current Fair Prices Migration
-- This migration modifies the rpe_fair_prices table to work as a current prices table
-- The fair prices are used by House bots for market making and liquidity provision

-- First, clean up duplicate entries by keeping only the most recent for each player
DELETE FROM rpe_fair_prices 
WHERE (player_id, ts) NOT IN (
    SELECT DISTINCT ON (player_id) 
        player_id, ts
    FROM rpe_fair_prices 
    ORDER BY player_id, ts DESC
);

-- Drop the existing primary key and create a new one on just player_id
ALTER TABLE rpe_fair_prices DROP CONSTRAINT IF EXISTS rpe_fair_prices_pkey;
ALTER TABLE rpe_fair_prices ADD PRIMARY KEY (player_id);

-- Add new columns for better tracking
ALTER TABLE rpe_fair_prices 
ADD COLUMN IF NOT EXISTS source TEXT DEFAULT 'projection',
ADD COLUMN IF NOT EXISTS confidence_score DECIMAL(3,2) DEFAULT 0.5;

-- Update existing records to have source information
UPDATE rpe_fair_prices 
SET source = CASE 
    WHEN reason->>'source' = 'live_points' THEN 'live_points'
    WHEN reason->>'source' = 'projection' THEN 'projection'
    ELSE 'hybrid'
END
WHERE source IS NULL;

-- Add comments for documentation
COMMENT ON TABLE rpe_fair_prices IS 'Current fair prices for each player, used by House bots for market making';
COMMENT ON COLUMN rpe_fair_prices.source IS 'Source of the fair price: projection, live_points, or hybrid';
COMMENT ON COLUMN rpe_fair_prices.confidence_score IS 'Confidence in the fair price (0.0 to 1.0)';
