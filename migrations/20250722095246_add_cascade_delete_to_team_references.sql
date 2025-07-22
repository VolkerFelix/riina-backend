-- Add ON DELETE CASCADE to foreign key constraints that reference teams table

-- 1. Update league_games constraints
ALTER TABLE league_games
    DROP CONSTRAINT IF EXISTS league_games_home_team_id_fkey,
    DROP CONSTRAINT IF EXISTS league_games_away_team_id_fkey,
    DROP CONSTRAINT IF EXISTS league_games_winner_team_id_fkey;

ALTER TABLE league_games
    ADD CONSTRAINT league_games_home_team_id_fkey
        FOREIGN KEY (home_team_id) REFERENCES teams(id) ON DELETE CASCADE,
    ADD CONSTRAINT league_games_away_team_id_fkey
        FOREIGN KEY (away_team_id) REFERENCES teams(id) ON DELETE CASCADE,
    ADD CONSTRAINT league_games_winner_team_id_fkey
        FOREIGN KEY (winner_team_id) REFERENCES teams(id) ON DELETE SET NULL;

-- 2. Update league_standings constraint
ALTER TABLE league_standings
    DROP CONSTRAINT IF EXISTS league_standings_team_id_fkey;

ALTER TABLE league_standings
    ADD CONSTRAINT league_standings_team_id_fkey
        FOREIGN KEY (team_id) REFERENCES teams(id) ON DELETE CASCADE;

-- 3. Update game_team_snapshots constraint
ALTER TABLE game_team_snapshots
    DROP CONSTRAINT IF EXISTS game_team_snapshots_team_id_fkey;

ALTER TABLE game_team_snapshots
    ADD CONSTRAINT game_team_snapshots_team_id_fkey
        FOREIGN KEY (team_id) REFERENCES teams(id) ON DELETE CASCADE;

-- 4. Update live_games constraints
ALTER TABLE live_games
    DROP CONSTRAINT IF EXISTS live_games_home_team_id_fkey,
    DROP CONSTRAINT IF EXISTS live_games_away_team_id_fkey;

ALTER TABLE live_games
    ADD CONSTRAINT live_games_home_team_id_fkey
        FOREIGN KEY (home_team_id) REFERENCES teams(id) ON DELETE CASCADE,
    ADD CONSTRAINT live_games_away_team_id_fkey
        FOREIGN KEY (away_team_id) REFERENCES teams(id) ON DELETE CASCADE;

-- 5. Update live_player_contributions constraint
ALTER TABLE live_player_contributions
    DROP CONSTRAINT IF EXISTS live_player_contributions_team_id_fkey;

ALTER TABLE live_player_contributions
    ADD CONSTRAINT live_player_contributions_team_id_fkey
        FOREIGN KEY (team_id) REFERENCES teams(id) ON DELETE CASCADE;