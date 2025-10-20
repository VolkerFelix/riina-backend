-- Update social features (reactions, comments) to support posts
-- Rename tables and add post_id support

BEGIN;

-- Rename tables to reflect post-centric architecture
ALTER TABLE workout_reactions RENAME TO post_reactions;
ALTER TABLE workout_comments RENAME TO post_comments;
ALTER TABLE comment_reactions RENAME TO post_comment_reactions;

-- Update renamed table: post_reactions
ALTER TABLE post_reactions
ADD COLUMN post_id UUID REFERENCES posts(id) ON DELETE CASCADE;

-- Rename workout_id column index
DROP INDEX IF EXISTS idx_workout_reactions_workout_id;
CREATE INDEX idx_post_reactions_workout_id ON post_reactions(workout_id) WHERE workout_id IS NOT NULL;

-- Add index for post reactions
CREATE INDEX idx_post_reactions_post_id ON post_reactions(post_id) WHERE post_id IS NOT NULL;

-- Update renamed table: post_comments
ALTER TABLE post_comments
ADD COLUMN post_id UUID REFERENCES posts(id) ON DELETE CASCADE;

-- Rename workout_id column index
DROP INDEX IF EXISTS idx_workout_comments_workout_id;
CREATE INDEX idx_post_comments_workout_id ON post_comments(workout_id) WHERE workout_id IS NOT NULL;

-- Add index for post comments
CREATE INDEX idx_post_comments_post_id ON post_comments(post_id) WHERE post_id IS NOT NULL;

-- Update foreign key in post_comment_reactions
ALTER TABLE post_comment_reactions
DROP CONSTRAINT IF EXISTS comment_reactions_comment_id_fkey;

ALTER TABLE post_comment_reactions
ADD CONSTRAINT post_comment_reactions_comment_id_fkey
    FOREIGN KEY (comment_id) REFERENCES post_comments(id) ON DELETE CASCADE;

-- Add constraints: must have either workout_id or post_id (not both)
ALTER TABLE post_reactions
ADD CONSTRAINT reactions_must_have_workout_or_post
    CHECK (
        (workout_id IS NOT NULL AND post_id IS NULL) OR
        (workout_id IS NULL AND post_id IS NOT NULL)
    );

ALTER TABLE post_comments
ADD CONSTRAINT comments_must_have_workout_or_post
    CHECK (
        (workout_id IS NOT NULL AND post_id IS NULL) OR
        (workout_id IS NULL AND post_id IS NOT NULL)
    );

-- Comments for documentation
COMMENT ON TABLE post_reactions IS 'User reactions (likes) on posts and workouts';
COMMENT ON TABLE post_comments IS 'User comments on posts and workouts';
COMMENT ON TABLE post_comment_reactions IS 'User reactions on comments';
COMMENT ON COLUMN post_reactions.post_id IS 'Reference to post (for unified posts). Mutually exclusive with workout_id.';
COMMENT ON COLUMN post_comments.post_id IS 'Reference to post (for unified posts). Mutually exclusive with workout_id.';

COMMIT;
