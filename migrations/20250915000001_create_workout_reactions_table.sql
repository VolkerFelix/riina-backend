-- Create workout reactions table for social engagement features
CREATE TABLE workout_reactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workout_id UUID NOT NULL REFERENCES workout_data(id) ON DELETE CASCADE,
    reaction_type VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure a user can only have one reaction type per workout
    CONSTRAINT unique_user_workout_reaction UNIQUE (user_id, workout_id)
);

-- Indexes for efficient queries
CREATE INDEX idx_workout_reactions_workout_id ON workout_reactions(workout_id);
CREATE INDEX idx_workout_reactions_user_id ON workout_reactions(user_id);
CREATE INDEX idx_workout_reactions_type ON workout_reactions(reaction_type);
CREATE INDEX idx_workout_reactions_created_at ON workout_reactions(created_at DESC);

-- Composite index for fetching reactions by workout with user info
CREATE INDEX idx_workout_reactions_workout_user ON workout_reactions(workout_id, user_id);

-- Index for counting reactions by type for a workout
CREATE INDEX idx_workout_reactions_workout_type ON workout_reactions(workout_id, reaction_type);