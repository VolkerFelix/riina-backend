-- Create live games management tables

-- Ensure week_end_date is properly set as TIMESTAMPTZ for existing games
DO $$ 
BEGIN
    -- Update week_end_date to end of day if it's set to start of day
    UPDATE league_games 
    SET week_end_date = week_end_date + INTERVAL '23 hours 59 minutes'
    WHERE week_end_date IS NOT NULL 
    AND DATE_PART('hour', week_end_date) = 0 
    AND DATE_PART('minute', week_end_date) = 0;
END $$;

-- Live score event type enum
CREATE TYPE live_score_event_type AS ENUM ('workout_upload', 'power_boost', 'team_bonus', 'milestone');

-- Main live games table
CREATE TABLE live_games (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id UUID NOT NULL REFERENCES league_games(id) ON DELETE CASCADE,
    home_team_id UUID NOT NULL REFERENCES teams(id),
    home_team_name VARCHAR(255) NOT NULL,
    away_team_id UUID NOT NULL REFERENCES teams(id),
    away_team_name VARCHAR(255) NOT NULL,
    home_score INTEGER NOT NULL DEFAULT 0,
    away_score INTEGER NOT NULL DEFAULT 0,
    home_power INTEGER NOT NULL DEFAULT 0,
    away_power INTEGER NOT NULL DEFAULT 0,
    game_start_time TIMESTAMPTZ NOT NULL,
    game_end_time TIMESTAMPTZ NOT NULL,
    last_score_time TIMESTAMPTZ,
    last_scorer_id UUID REFERENCES users(id),
    last_scorer_name VARCHAR(255),
    last_scorer_team VARCHAR(10), -- 'home' or 'away'
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Player contributions in live games
CREATE TABLE live_player_contributions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    live_game_id UUID NOT NULL REFERENCES live_games(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id),
    username VARCHAR(255) NOT NULL,
    team_id UUID NOT NULL REFERENCES teams(id),
    team_name VARCHAR(255) NOT NULL,
    team_side VARCHAR(10) NOT NULL, -- 'home' or 'away'
    current_power INTEGER NOT NULL DEFAULT 0,
    total_score_contribution INTEGER NOT NULL DEFAULT 0,
    last_contribution_time TIMESTAMPTZ,
    contribution_count INTEGER NOT NULL DEFAULT 0,
    is_currently_active BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Score events during live games
CREATE TABLE live_score_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    live_game_id UUID NOT NULL REFERENCES live_games(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id),
    username VARCHAR(255) NOT NULL,
    team_id UUID NOT NULL REFERENCES teams(id),
    team_side VARCHAR(10) NOT NULL, -- 'home' or 'away'
    score_points INTEGER NOT NULL,
    power_contribution INTEGER NOT NULL,
    stamina_gained INTEGER NOT NULL DEFAULT 0,
    strength_gained INTEGER NOT NULL DEFAULT 0,
    event_type live_score_event_type NOT NULL,
    description TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_live_games_game_id ON live_games(game_id);
CREATE INDEX idx_live_games_active ON live_games(is_active, game_start_time);
CREATE INDEX idx_live_games_teams ON live_games(home_team_id, away_team_id);

CREATE INDEX idx_live_player_contributions_live_game ON live_player_contributions(live_game_id);
CREATE INDEX idx_live_player_contributions_user ON live_player_contributions(user_id);
CREATE INDEX idx_live_player_contributions_team ON live_player_contributions(team_id, team_side);
CREATE INDEX idx_live_player_contributions_active ON live_player_contributions(is_currently_active, last_contribution_time);

CREATE INDEX idx_live_score_events_live_game ON live_score_events(live_game_id);
CREATE INDEX idx_live_score_events_user ON live_score_events(user_id);
CREATE INDEX idx_live_score_events_time ON live_score_events(occurred_at DESC);
CREATE INDEX idx_live_score_events_team_side ON live_score_events(team_side, occurred_at DESC);

-- Unique constraint to prevent duplicate live games
CREATE UNIQUE INDEX idx_live_games_unique_active ON live_games(game_id) WHERE is_active = true;

-- Unique constraint for player contributions per live game
CREATE UNIQUE INDEX idx_live_player_contributions_unique ON live_player_contributions(live_game_id, user_id);