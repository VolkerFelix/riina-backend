-- Rename health_data table to workout_data and add workout analysis fields
-- The current health_data table is really storing workout data, so let's rename it properly

-- First, rename the existing table
ALTER TABLE health_data RENAME TO workout_data;

-- Update existing indexes
DROP INDEX IF EXISTS idx_health_data_user_id;
DROP INDEX IF EXISTS idx_health_data_device_id;
DROP INDEX IF EXISTS idx_health_data_workout_start;
DROP INDEX IF EXISTS idx_health_data_workout_uuid;
DROP INDEX IF EXISTS idx_health_data_workout_uuid_lookup;

-- Rename existing columns to be more descriptive
ALTER TABLE workout_data RENAME COLUMN active_energy_burned TO calories_burned;

-- Add new workout analysis fields
ALTER TABLE workout_data 
ADD COLUMN duration_minutes INTEGER,
ADD COLUMN avg_heart_rate INTEGER,
ADD COLUMN max_heart_rate INTEGER,
ADD COLUMN min_heart_rate INTEGER,
ADD COLUMN heart_rate_zones JSONB,
ADD COLUMN stamina_gained INTEGER NOT NULL DEFAULT 0,
ADD COLUMN strength_gained INTEGER NOT NULL DEFAULT 0,
ADD COLUMN total_points_gained INTEGER NOT NULL DEFAULT 0,
ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

-- Create new indexes with proper naming
CREATE INDEX idx_workout_data_user_id ON workout_data(user_id);
CREATE INDEX idx_workout_data_device_id ON workout_data(device_id);
CREATE INDEX idx_workout_data_workout_start ON workout_data(workout_start);
CREATE INDEX idx_workout_data_created_at ON workout_data(created_at);
CREATE INDEX idx_workout_data_workout_uuid ON workout_data(workout_uuid) WHERE workout_uuid IS NOT NULL;

-- Composite index for efficient time-based duplicate checks
CREATE INDEX idx_workout_data_user_time_check ON workout_data(user_id, workout_start, workout_end) 
WHERE workout_start IS NOT NULL AND workout_end IS NOT NULL;