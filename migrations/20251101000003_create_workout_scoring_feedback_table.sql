-- Create table for workout scoring feedback
-- Allows users to rate if the scoring represents their suffering level

CREATE TABLE workout_scoring_feedback (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workout_data_id UUID NOT NULL REFERENCES workout_data(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    feedback_type VARCHAR(20) NOT NULL CHECK (feedback_type IN ('too_high', 'too_low', 'accurate')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure one feedback per user per workout
    UNIQUE (workout_data_id, user_id)
);

-- Index for finding feedback by workout
CREATE INDEX idx_workout_scoring_feedback_workout_id ON workout_scoring_feedback(workout_data_id);

-- Index for finding feedback by user
CREATE INDEX idx_workout_scoring_feedback_user_id ON workout_scoring_feedback(user_id);

-- Index for analytics queries
CREATE INDEX idx_workout_scoring_feedback_type ON workout_scoring_feedback(feedback_type);
