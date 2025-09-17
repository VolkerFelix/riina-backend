-- Create workout comments table for social engagement features
CREATE TABLE workout_comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workout_id UUID NOT NULL REFERENCES workout_data(id) ON DELETE CASCADE,
    parent_id UUID REFERENCES workout_comments(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    is_edited BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for efficient queries
CREATE INDEX idx_workout_comments_workout_id ON workout_comments(workout_id);
CREATE INDEX idx_workout_comments_user_id ON workout_comments(user_id);
CREATE INDEX idx_workout_comments_parent_id ON workout_comments(parent_id);
CREATE INDEX idx_workout_comments_created_at ON workout_comments(created_at DESC);

-- Composite index for fetching top-level comments (replies have parent_id)
CREATE INDEX idx_workout_comments_workout_toplevel ON workout_comments(workout_id, created_at DESC)
WHERE parent_id IS NULL;

-- Index for fetching replies to a comment
CREATE INDEX idx_workout_comments_replies ON workout_comments(parent_id, created_at ASC)
WHERE parent_id IS NOT NULL;

-- Trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_workout_comments_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    NEW.is_edited = TRUE;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER update_workout_comments_updated_at_trigger
BEFORE UPDATE OF content ON workout_comments
FOR EACH ROW
EXECUTE FUNCTION update_workout_comments_updated_at();