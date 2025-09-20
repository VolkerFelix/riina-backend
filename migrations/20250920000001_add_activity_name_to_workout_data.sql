-- Add activity_name column to workout_data table to store the type of workout activity
ALTER TABLE workout_data
ADD COLUMN activity_name VARCHAR(255) NULL;

-- Create index for querying workouts by activity type
CREATE INDEX idx_workout_data_activity_name ON workout_data(activity_name) WHERE activity_name IS NOT NULL;