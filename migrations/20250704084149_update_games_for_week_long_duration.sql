-- Update league_games table to support week-long games
ALTER TABLE league_games 
ADD COLUMN week_start_date DATE,
ADD COLUMN week_end_date DATE;

-- Update existing games to have week dates based on scheduled_time
UPDATE league_games 
SET 
    week_start_date = DATE(scheduled_time - INTERVAL '6 days'),
    week_end_date = DATE(scheduled_time)
WHERE week_start_date IS NULL;

-- Add constraint to ensure week dates are valid
ALTER TABLE league_games 
ADD CONSTRAINT valid_week_dates CHECK (week_end_date >= week_start_date);

-- Update the status check constraint to include 'in_progress' status
ALTER TABLE league_games 
DROP CONSTRAINT IF EXISTS valid_status;

ALTER TABLE league_games 
ADD CONSTRAINT valid_status CHECK (status IN ('scheduled', 'in_progress', 'finished', 'postponed'));

-- Add indexes for performance
CREATE INDEX idx_league_games_week_dates ON league_games(week_start_date, week_end_date);
CREATE INDEX idx_league_games_status ON league_games(status) WHERE status = 'in_progress';