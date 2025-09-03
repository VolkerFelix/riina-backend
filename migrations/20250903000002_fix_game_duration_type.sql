-- Fix game_duration_seconds to use BIGINT type instead of DOUBLE PRECISION

-- Drop the existing constraint
ALTER TABLE league_seasons 
DROP CONSTRAINT IF EXISTS check_game_duration_seconds;

-- Change the column type from DOUBLE PRECISION to BIGINT and convert values
ALTER TABLE league_seasons 
ALTER COLUMN game_duration_seconds TYPE BIGINT USING (game_duration_seconds)::BIGINT;

-- Add new constraint to ensure reasonable game durations (1 second to 30 days in seconds)
ALTER TABLE league_seasons 
ADD CONSTRAINT check_game_duration_seconds 
CHECK (game_duration_seconds >= 1 AND game_duration_seconds <= 2592000);

-- Update comment for documentation
COMMENT ON COLUMN league_seasons.game_duration_seconds IS 'Duration of each game in seconds (BIGINT). Default is 518400 seconds (6 days). Minimum 1 second, maximum 2592000 seconds (30 days).';