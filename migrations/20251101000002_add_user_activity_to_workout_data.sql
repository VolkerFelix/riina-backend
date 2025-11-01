-- Add user_activity column to workout_data for user-edited activity types
-- This field takes precedence over activity_name when displaying in the app

ALTER TABLE workout_data
ADD COLUMN user_activity VARCHAR(255);

-- Create index for efficient filtering by user_activity
CREATE INDEX idx_workout_data_user_activity ON workout_data(user_activity) WHERE user_activity IS NOT NULL;

-- Add comment explaining the fields
COMMENT ON COLUMN workout_data.activity_name IS 'Original activity name from health data source (read-only)';
COMMENT ON COLUMN workout_data.user_activity IS 'User-edited activity type (takes precedence over activity_name when set)';
