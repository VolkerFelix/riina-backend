-- Make workout_uuid required and unique to prevent duplicate uploads
-- This ensures all workout uploads must have a unique identifier from the source (e.g., Apple Health)

-- First, clean up any existing NULL workout_uuid records (these are likely duplicates)
DELETE FROM workout_data WHERE workout_uuid IS NULL;

-- Drop the existing regular index
DROP INDEX IF EXISTS idx_workout_data_workout_uuid;
DROP INDEX IF EXISTS idx_workout_data_workout_uuid_lookup;

-- Make workout_uuid NOT NULL and add UNIQUE constraint
ALTER TABLE workout_data 
ALTER COLUMN workout_uuid SET NOT NULL,
ADD CONSTRAINT unique_workout_uuid UNIQUE (workout_uuid);

-- Create index for performance
CREATE INDEX idx_workout_data_workout_uuid ON workout_data(workout_uuid);