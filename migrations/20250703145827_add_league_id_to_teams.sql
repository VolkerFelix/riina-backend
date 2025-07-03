-- Add league_id column to teams table to establish direct relationship
-- Teams should belong directly to a league, not just through seasons

-- Add the league_id column as nullable initially
ALTER TABLE teams 
ADD COLUMN league_id UUID REFERENCES leagues(id) ON DELETE SET NULL;

-- Create index for better performance
CREATE INDEX idx_teams_league_id ON teams(league_id);

-- For existing teams, we can set them to belong to the league that has active seasons
-- This is a data migration to handle existing data
UPDATE teams 
SET league_id = (
    SELECT DISTINCT ls.league_id 
    FROM league_seasons ls 
    JOIN league_teams lt ON ls.id = lt.season_id 
    WHERE lt.team_id = teams.id 
    LIMIT 1
)
WHERE EXISTS (
    SELECT 1 
    FROM league_seasons ls 
    JOIN league_teams lt ON ls.id = lt.season_id 
    WHERE lt.team_id = teams.id
);