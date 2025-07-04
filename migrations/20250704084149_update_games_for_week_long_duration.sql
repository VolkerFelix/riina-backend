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

-- Create table to store team score snapshots at game start and end
CREATE TABLE IF NOT EXISTS game_team_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id UUID NOT NULL REFERENCES league_games(id) ON DELETE CASCADE,
    team_id UUID NOT NULL REFERENCES teams(id),
    snapshot_type VARCHAR(20) NOT NULL CHECK (snapshot_type IN ('start', 'end')),
    total_stamina INTEGER NOT NULL DEFAULT 0,
    total_strength INTEGER NOT NULL DEFAULT 0,
    member_count INTEGER NOT NULL DEFAULT 0,
    snapshot_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Ensure only one snapshot per game/team/type
    UNIQUE(game_id, team_id, snapshot_type)
);

-- Add indexes for performance
CREATE INDEX IF NOT EXISTS idx_league_games_week_dates ON league_games(week_start_date, week_end_date);
CREATE INDEX IF NOT EXISTS idx_league_games_status_in_progress ON league_games(status) WHERE status = 'in_progress';
CREATE INDEX IF NOT EXISTS idx_game_team_snapshots_game ON game_team_snapshots(game_id);
CREATE INDEX IF NOT EXISTS idx_game_team_snapshots_team ON game_team_snapshots(team_id);