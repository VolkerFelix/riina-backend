-- Consolidate league_games and live_games into a single games table
-- This migration combines both tables and adds live scoring columns to league_games

BEGIN;

-- First, add live scoring columns to league_games table
ALTER TABLE league_games 
ADD COLUMN home_score INTEGER NOT NULL DEFAULT 0,
ADD COLUMN away_score INTEGER NOT NULL DEFAULT 0,
ADD COLUMN game_start_time TIMESTAMPTZ,
ADD COLUMN game_end_time TIMESTAMPTZ,
ADD COLUMN last_score_time TIMESTAMPTZ,
ADD COLUMN last_scorer_id UUID,
ADD COLUMN last_scorer_name VARCHAR(255),
ADD COLUMN last_scorer_team VARCHAR(10); -- 'home' or 'away'

-- Migrate data from live_games to league_games
UPDATE league_games 
SET 
    home_score = lg.home_score,
    away_score = lg.away_score,
    game_start_time = lg.game_start_time,
    game_end_time = lg.game_end_time,
    last_score_time = lg.last_score_time,
    last_scorer_id = lg.last_scorer_id,
    last_scorer_name = lg.last_scorer_name,
    last_scorer_team = lg.last_scorer_team
FROM live_games lg
WHERE league_games.id = lg.game_id;

-- Update live_score_events to reference league_games directly instead of live_games
-- First add the new column
ALTER TABLE live_score_events ADD COLUMN game_id UUID;

-- Populate the new column with the corresponding game_id from live_games
UPDATE live_score_events 
SET game_id = lg.game_id
FROM live_games lg
WHERE live_score_events.live_game_id = lg.id;

-- Drop the old constraint and column, add new constraint
ALTER TABLE live_score_events
DROP CONSTRAINT IF EXISTS live_score_events_live_game_id_fkey;

ALTER TABLE live_score_events
DROP COLUMN live_game_id;

ALTER TABLE live_score_events
ADD CONSTRAINT live_score_events_game_id_fkey 
    FOREIGN KEY (game_id) REFERENCES league_games(id) ON DELETE CASCADE;

-- Drop dependent tables first
DROP TABLE IF EXISTS live_player_contributions;
DROP TABLE IF EXISTS live_games;

-- Rename league_games to games for simplicity
ALTER TABLE league_games RENAME TO games;

-- Update any constraints that reference the old table name
ALTER INDEX IF EXISTS league_games_pkey RENAME TO games_pkey;
ALTER INDEX IF EXISTS league_games_season_id_idx RENAME TO games_season_id_idx;
ALTER INDEX IF EXISTS league_games_home_team_id_idx RENAME TO games_home_team_id_idx;
ALTER INDEX IF EXISTS league_games_away_team_id_idx RENAME TO games_away_team_id_idx;

-- Update foreign key constraints to reference the new table name
ALTER TABLE live_score_events
DROP CONSTRAINT IF EXISTS live_score_events_game_id_fkey,
ADD CONSTRAINT live_score_events_game_id_fkey 
    FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE;

-- Add helpful indexes for the new columns
CREATE INDEX IF NOT EXISTS games_status_idx ON games(status);
CREATE INDEX IF NOT EXISTS games_game_start_time_idx ON games(game_start_time);
CREATE INDEX IF NOT EXISTS games_game_end_time_idx ON games(game_end_time);

COMMIT;