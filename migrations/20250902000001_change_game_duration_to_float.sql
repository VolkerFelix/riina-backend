-- Change game_duration_minutes from INTEGER to DOUBLE PRECISION to support floating point values

-- Drop the existing constraint
ALTER TABLE league_seasons 
DROP CONSTRAINT IF EXISTS check_game_duration_minutes;

-- Change the column type from INTEGER to DOUBLE PRECISION to support fractional minutes
-- DOUBLE PRECISION maps directly to Rust's f64 type
ALTER TABLE league_seasons 
ALTER COLUMN game_duration_minutes TYPE DOUBLE PRECISION USING game_duration_minutes::DOUBLE PRECISION;

-- Add updated constraint to ensure reasonable game durations (0.001 minute to 30 days)
ALTER TABLE league_seasons 
ADD CONSTRAINT check_game_duration_minutes 
CHECK (game_duration_minutes >= 0.001 AND game_duration_minutes <= 43200.0);

-- Update comment for documentation
COMMENT ON COLUMN league_seasons.game_duration_minutes IS 'Duration of each game in minutes (supports fractional minutes). Default is 8640.0 minutes (6 days). Minimum 0.001 minute, maximum 43200.0 minutes (30 days).';