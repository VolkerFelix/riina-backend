-- Add ON DELETE CASCADE to foreign key constraints that reference users table

-- Drop and recreate foreign key constraint for live_player_contributions
ALTER TABLE live_player_contributions
    DROP CONSTRAINT IF EXISTS live_player_contributions_user_id_fkey;

ALTER TABLE live_player_contributions
    ADD CONSTRAINT live_player_contributions_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

-- Drop and recreate foreign key constraint for live_games.last_scorer_id
-- This one should be SET NULL instead of CASCADE since it's nullable
ALTER TABLE live_games
    DROP CONSTRAINT IF EXISTS live_games_last_scorer_id_fkey;

ALTER TABLE live_games
    ADD CONSTRAINT live_games_last_scorer_id_fkey
    FOREIGN KEY (last_scorer_id) REFERENCES users(id) ON DELETE SET NULL;