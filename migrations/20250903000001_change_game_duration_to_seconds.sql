-- Change game_duration_minutes to game_duration_seconds for consistency

-- Drop the existing constraint
ALTER TABLE league_seasons 
DROP CONSTRAINT IF EXISTS check_game_duration_minutes;

-- Rename the column and convert minutes to seconds
ALTER TABLE league_seasons 
RENAME COLUMN game_duration_minutes TO game_duration_seconds;

-- Convert the values from minutes to seconds (multiply by 60)
UPDATE league_seasons 
SET game_duration_seconds = game_duration_seconds * 60;

-- Add new constraint to ensure reasonable game durations (1 second to 30 days in seconds)
ALTER TABLE league_seasons 
ADD CONSTRAINT check_game_duration_seconds 
CHECK (game_duration_seconds >= 1 AND game_duration_seconds <= 2592000);

-- Update comment for documentation
COMMENT ON COLUMN league_seasons.game_duration_seconds IS 'Duration of each game in seconds. Default is 518400 seconds (6 days). Minimum 1 second, maximum 2592000 seconds (30 days).';