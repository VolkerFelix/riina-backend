-- Add game duration field to league_seasons table to allow configurable game durations per season

ALTER TABLE league_seasons 
ADD COLUMN game_duration_minutes INTEGER DEFAULT 8640 NOT NULL; -- Default: 6 days = 8640 minutes

-- Add a constraint to ensure reasonable game durations (1 minute to 30 days)
ALTER TABLE league_seasons 
ADD CONSTRAINT check_game_duration_minutes 
CHECK (game_duration_minutes >= 1 AND game_duration_minutes <= 43200); -- 1 min to 30 days

-- Add comment for documentation
COMMENT ON COLUMN league_seasons.game_duration_minutes IS 'Duration of each game in minutes. Default is 8640 minutes (6 days). Minimum 1 minute, maximum 43200 minutes (30 days).';