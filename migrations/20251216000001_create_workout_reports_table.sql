-- Create table for suspicious workout reports
-- Allows users to report their own or others' workouts as suspicious

CREATE TABLE IF NOT EXISTS workout_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workout_data_id UUID NOT NULL REFERENCES workout_data(id) ON DELETE CASCADE,
    reported_by_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workout_owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reason TEXT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'reviewed', 'dismissed', 'confirmed')),
    admin_notes TEXT,
    reviewed_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    reviewed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure one report per user per workout
    UNIQUE (workout_data_id, reported_by_user_id)
);

-- Index for finding reports by workout
CREATE INDEX IF NOT EXISTS idx_workout_reports_workout_id ON workout_reports(workout_data_id);

-- Index for finding reports by reporter
CREATE INDEX IF NOT EXISTS idx_workout_reports_reporter ON workout_reports(reported_by_user_id);

-- Index for finding reports by workout owner
CREATE INDEX IF NOT EXISTS idx_workout_reports_owner ON workout_reports(workout_owner_id);

-- Index for finding reports by status
CREATE INDEX IF NOT EXISTS idx_workout_reports_status ON workout_reports(status);

-- Index for admin review queries
CREATE INDEX IF NOT EXISTS idx_workout_reports_pending ON workout_reports(status, created_at) WHERE status = 'pending';
