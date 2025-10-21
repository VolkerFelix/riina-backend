-- Add indexes to optimize feed query performance
-- Addresses N+1 query problem and improves feed load times

BEGIN;

-- Index on posts table for feed queries
-- Covers the WHERE clause: visibility = 'public' AND created_at < cursor
CREATE INDEX IF NOT EXISTS idx_posts_feed_query
    ON posts(visibility, created_at DESC)
    WHERE visibility = 'public';

-- Index on post_reactions for workout_id lookups (used in CTEs)
CREATE INDEX IF NOT EXISTS idx_post_reactions_workout_id
    ON post_reactions(workout_id)
    WHERE workout_id IS NOT NULL;

-- Index on post_reactions for user reactions lookup
CREATE INDEX IF NOT EXISTS idx_post_reactions_user_workout
    ON post_reactions(user_id, workout_id)
    WHERE workout_id IS NOT NULL;

-- Index on post_comments for workout_id lookups (used in CTEs)
CREATE INDEX IF NOT EXISTS idx_post_comments_workout_id
    ON post_comments(workout_id)
    WHERE workout_id IS NOT NULL;

-- Index on posts.workout_id for the LEFT JOIN
CREATE INDEX IF NOT EXISTS idx_posts_workout_id
    ON posts(workout_id)
    WHERE workout_id IS NOT NULL;

-- Comments for documentation
COMMENT ON INDEX idx_posts_feed_query IS 'Optimizes feed queries filtering by visibility and ordering by created_at';
COMMENT ON INDEX idx_post_reactions_workout_id IS 'Speeds up reaction count aggregation in feed CTEs';
COMMENT ON INDEX idx_post_reactions_user_workout IS 'Optimizes user reaction status lookup in feed';
COMMENT ON INDEX idx_post_comments_workout_id IS 'Speeds up comment count aggregation in feed CTEs';
COMMENT ON INDEX idx_posts_workout_id IS 'Optimizes JOIN with workout_data table';

COMMIT;
