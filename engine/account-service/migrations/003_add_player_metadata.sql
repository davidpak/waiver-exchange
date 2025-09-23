-- Migration: 003_add_player_metadata.sql
-- This migration creates the player_metadata table for caching player information
-- Created: 2024-01-15
-- Purpose: Cache player metadata with assigned symbol IDs for fast API access

-- Player metadata table for caching player information
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

-- Indexes for fast lookups
CREATE INDEX idx_player_metadata_symbol_id ON player_metadata(symbol_id);
CREATE INDEX idx_player_metadata_name ON player_metadata(name);
CREATE INDEX idx_player_metadata_position ON player_metadata(position);
CREATE INDEX idx_player_metadata_team ON player_metadata(team);
CREATE INDEX idx_player_metadata_rank ON player_metadata(rank);

-- Add comments for documentation
COMMENT ON TABLE player_metadata IS 'Cached player metadata with assigned symbol IDs';
COMMENT ON COLUMN player_metadata.player_id IS 'Sleeper player ID (primary key)';
COMMENT ON COLUMN player_metadata.name IS 'Player full name';
COMMENT ON COLUMN player_metadata.position IS 'Player position (QB, RB, WR, TE, etc.)';
COMMENT ON COLUMN player_metadata.team IS 'Player team abbreviation';
COMMENT ON COLUMN player_metadata.projected_points IS 'Projected fantasy points for season';
COMMENT ON COLUMN player_metadata.rank IS 'Player rank by projected points';
COMMENT ON COLUMN player_metadata.symbol_id IS 'Assigned symbol ID for trading';
COMMENT ON COLUMN player_metadata.last_updated IS 'Last time this record was updated';
