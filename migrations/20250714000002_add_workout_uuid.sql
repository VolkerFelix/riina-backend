-- Add workout_uuid column to health_data table for duplicate prevention
ALTER TABLE health_data 
ADD COLUMN workout_uuid VARCHAR(255);

-- Create unique index to prevent duplicate workouts (allowing null values for non-workout health data)
CREATE UNIQUE INDEX idx_health_data_workout_uuid ON health_data(workout_uuid) WHERE workout_uuid IS NOT NULL;

-- Create index for faster lookups
CREATE INDEX IF NOT EXISTS idx_health_data_workout_uuid_lookup ON health_data(workout_uuid);