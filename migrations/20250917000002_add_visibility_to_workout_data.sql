-- Add visibility field to workout_data for newsfeed privacy control
ALTER TABLE workout_data
ADD COLUMN visibility VARCHAR(20) NOT NULL DEFAULT 'public'
CHECK (visibility IN ('public', 'private', 'friends'));

-- Create index for efficient feed queries
CREATE INDEX idx_workout_data_visibility ON workout_data(visibility);

-- Composite index for feed queries (public workouts ordered by date)
CREATE INDEX idx_workout_data_feed ON workout_data(visibility, created_at DESC)
WHERE visibility = 'public';

-- Add index for user-specific public workouts
CREATE INDEX idx_workout_data_user_public ON workout_data(user_id, visibility, created_at DESC)
WHERE visibility = 'public';