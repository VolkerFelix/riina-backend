-- Add is_duplicate flag to track workouts that are duplicates based on time overlap
-- This allows us to accept the workout (store the UUID) but not count it for stats

ALTER TABLE workout_data 
ADD COLUMN is_duplicate BOOLEAN NOT NULL DEFAULT FALSE;

-- Add index for finding non-duplicate workouts efficiently
CREATE INDEX idx_workout_data_is_duplicate ON workout_data(is_duplicate) WHERE is_duplicate = FALSE;

-- Add comment explaining the column
COMMENT ON COLUMN workout_data.is_duplicate IS 'True if this workout is a duplicate of another workout based on time overlap (15 second tolerance). Duplicates are stored to prevent repeated upload attempts but do not contribute to stats.';