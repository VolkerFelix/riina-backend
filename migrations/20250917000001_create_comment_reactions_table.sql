-- Create comment reactions table for social engagement features
CREATE TABLE IF NOT EXISTS comment_reactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    comment_id UUID NOT NULL REFERENCES workout_comments(id) ON DELETE CASCADE,
    reaction_type VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure a user can only have one reaction type per comment
    CONSTRAINT unique_user_comment_reaction UNIQUE (user_id, comment_id)
);

-- Indexes for efficient queries (using IF NOT EXISTS for safety)
CREATE INDEX IF NOT EXISTS idx_comment_reactions_comment_id ON comment_reactions(comment_id);
CREATE INDEX IF NOT EXISTS idx_comment_reactions_user_id ON comment_reactions(user_id);
CREATE INDEX IF NOT EXISTS idx_comment_reactions_type ON comment_reactions(reaction_type);
CREATE INDEX IF NOT EXISTS idx_comment_reactions_created_at ON comment_reactions(created_at DESC);

-- Composite index for fetching reactions by comment with user info
CREATE INDEX IF NOT EXISTS idx_comment_reactions_comment_user ON comment_reactions(comment_id, user_id);

-- Index for counting reactions by type for a comment
CREATE INDEX IF NOT EXISTS idx_comment_reactions_comment_type ON comment_reactions(comment_id, reaction_type);
