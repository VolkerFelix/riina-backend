-- Add games_per_matchup field to league_seasons table
-- This field controls whether teams play each other once (1) or twice (2) in a season

ALTER TABLE league_seasons 
ADD COLUMN games_per_matchup INTEGER DEFAULT 1;

-- Add constraint to ensure valid values (1 = single round-robin, 2 = double round-robin)
ALTER TABLE league_seasons 
ADD CONSTRAINT check_games_per_matchup 
CHECK (games_per_matchup >= 1 AND games_per_matchup <= 2);

-- Update comment for documentation
COMMENT ON COLUMN league_seasons.games_per_matchup IS 'Number of games per team matchup: 1 = single round-robin (each team plays every other team once), 2 = double round-robin (each team plays every other team twice). Default is 1.';