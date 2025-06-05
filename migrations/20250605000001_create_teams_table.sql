-- Create teams table for league team registration
CREATE TABLE IF NOT EXISTS teams (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    team_name VARCHAR(50) NOT NULL,
    team_description TEXT,
    team_color VARCHAR(7) NOT NULL DEFAULT '#4F46E5', -- Hex color code
    team_icon VARCHAR(10) NOT NULL DEFAULT '⚽', -- Emoji or icon
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Constraints
    CONSTRAINT unique_user_team UNIQUE(user_id), -- One team per user
    CONSTRAINT unique_team_name UNIQUE(team_name), -- Unique team names
    CONSTRAINT valid_team_name CHECK (LENGTH(TRIM(team_name)) >= 2),
    CONSTRAINT valid_team_color CHECK (team_color ~ '^#[0-9A-Fa-f]{6}$'), -- Valid hex color
    CONSTRAINT valid_team_description CHECK (team_description IS NULL OR LENGTH(team_description) <= 500),
    CONSTRAINT non_empty_team_name CHECK (TRIM(team_name) != '')
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_teams_user_id ON teams(user_id);
CREATE INDEX IF NOT EXISTS idx_teams_team_name ON teams(team_name);
CREATE INDEX IF NOT EXISTS idx_teams_created_at ON teams(created_at);

-- Add some sample team colors and icons as reference (commented out)
-- Common team colors: #FF0000 (red), #00FF00 (green), #0000FF (blue), #FFFF00 (yellow)
-- Common team icons: ⚽ 🏆 🎯 ⭐ 🔥 ⚡ 🛡️ 🦅 🦁 🐺

-- Insert trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_teams_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_teams_updated_at
    BEFORE UPDATE ON teams
    FOR EACH ROW
    EXECUTE FUNCTION update_teams_updated_at();