-- Create player_pool table for active users not currently on a team
-- This table tracks players available for recruitment

CREATE TABLE IF NOT EXISTS player_pool (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    league_id UUID REFERENCES leagues(id) ON DELETE CASCADE, -- Optional: filter by league
    joined_pool_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_active_at TIMESTAMPTZ NOT NULL DEFAULT NOW() -- Track user activity
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_player_pool_user_id ON player_pool(user_id);
CREATE INDEX IF NOT EXISTS idx_player_pool_league_id ON player_pool(league_id);
CREATE INDEX IF NOT EXISTS idx_player_pool_last_active_at ON player_pool(last_active_at);