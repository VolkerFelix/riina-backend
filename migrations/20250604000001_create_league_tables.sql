-- League seasons
CREATE TABLE IF NOT EXISTS league_seasons (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    start_date TIMESTAMPTZ NOT NULL,
    end_date TIMESTAMPTZ NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- League games/matches
CREATE TABLE IF NOT EXISTS league_games (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    season_id UUID NOT NULL REFERENCES league_seasons(id) ON DELETE CASCADE,
    home_team_id UUID NOT NULL,
    away_team_id UUID NOT NULL,
    scheduled_time TIMESTAMPTZ NOT NULL, -- Always Saturday 10pm
    week_number INTEGER NOT NULL,
    is_first_leg BOOLEAN NOT NULL DEFAULT TRUE, -- True for first meeting, false for second
    status VARCHAR(50) NOT NULL DEFAULT 'scheduled', -- scheduled, live, finished, postponed
    winner_team_id UUID NULL,
    home_score INTEGER,
    away_score INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT valid_status CHECK (status IN ('scheduled', 'live', 'finished', 'postponed')),
    CONSTRAINT different_teams CHECK (home_team_id != away_team_id)
);

-- League standings
CREATE TABLE IF NOT EXISTS league_standings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    season_id UUID NOT NULL REFERENCES league_seasons(id) ON DELETE CASCADE,
    team_id UUID NOT NULL,
    games_played INTEGER NOT NULL DEFAULT 0,
    wins INTEGER NOT NULL DEFAULT 0,
    draws INTEGER NOT NULL DEFAULT 0,
    losses INTEGER NOT NULL DEFAULT 0,
    points INTEGER GENERATED ALWAYS AS (wins * 3 + draws) STORED,
    position INTEGER NOT NULL DEFAULT 1,
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(season_id, team_id)
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_league_games_season_id ON league_games(season_id);
CREATE INDEX IF NOT EXISTS idx_league_games_scheduled_time ON league_games(scheduled_time);
CREATE INDEX IF NOT EXISTS idx_league_games_status ON league_games(status);
CREATE INDEX IF NOT EXISTS idx_league_games_teams ON league_games(home_team_id, away_team_id);
CREATE INDEX IF NOT EXISTS idx_league_standings_season ON league_standings(season_id, points DESC);