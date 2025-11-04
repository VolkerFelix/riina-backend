-- Remove feedback_type column from workout_scoring_feedback table
-- We only need effort_rating (0-10 scale)

-- Drop the feedback_type column if it exists
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'workout_scoring_feedback'
        AND column_name = 'feedback_type'
    ) THEN
        ALTER TABLE workout_scoring_feedback DROP COLUMN feedback_type;
    END IF;
END $$;

-- Drop the feedback_type index if it exists
DROP INDEX IF EXISTS idx_workout_scoring_feedback_type;
