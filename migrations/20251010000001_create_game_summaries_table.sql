-- Create game summaries table to store post-game statistics and analysis
-- This table is populated when a game transitions from 'finished' to 'evaluated'

BEGIN;

CREATE TABLE IF NOT EXISTS game_summaries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,

    -- Game Overview
    final_home_score INTEGER NOT NULL,
    final_away_score INTEGER NOT NULL,
    game_start_date TIMESTAMPTZ NOT NULL,
    game_end_date TIMESTAMPTZ NOT NULL,
    mvp_user_id UUID REFERENCES users(id),
    mvp_username VARCHAR(255),
    mvp_team_id UUID REFERENCES teams(id),
    mvp_score_contribution INTEGER,
    lvp_user_id UUID REFERENCES users(id),
    lvp_username VARCHAR(255),
    lvp_team_id UUID REFERENCES teams(id),
    lvp_score_contribution INTEGER,

    -- Home Team Statistics
    home_team_avg_score_per_player REAL,
    home_team_total_workouts INTEGER NOT NULL DEFAULT 0,
    home_team_top_scorer_id UUID REFERENCES users(id),
    home_team_top_scorer_username VARCHAR(255),
    home_team_top_scorer_points INTEGER,
    home_team_lowest_performer_id UUID REFERENCES users(id),
    home_team_lowest_performer_username VARCHAR(255),
    home_team_lowest_performer_points INTEGER,

    -- Away Team Statistics
    away_team_avg_score_per_player REAL,
    away_team_total_workouts INTEGER NOT NULL DEFAULT 0,
    away_team_top_scorer_id UUID REFERENCES users(id),
    away_team_top_scorer_username VARCHAR(255),
    away_team_top_scorer_points INTEGER,
    away_team_lowest_performer_id UUID REFERENCES users(id),
    away_team_lowest_performer_username VARCHAR(255),
    away_team_lowest_performer_points INTEGER,

    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_game_summaries_game_id ON game_summaries(game_id);
CREATE INDEX IF NOT EXISTS idx_game_summaries_mvp ON game_summaries(mvp_user_id);
CREATE INDEX IF NOT EXISTS idx_game_summaries_created_at ON game_summaries(created_at DESC);

-- Ensure only one summary per game
CREATE UNIQUE INDEX IF NOT EXISTS idx_game_summaries_unique_game ON game_summaries(game_id);

COMMIT;
