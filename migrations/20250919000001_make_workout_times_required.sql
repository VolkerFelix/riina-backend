-- Make workout_start and workout_end required (non-nullable)
-- First, update any existing records that might have NULL values

-- Update any NULL workout_start times to use the created_at (fallback)
UPDATE workout_data
SET workout_start = created_at
WHERE workout_start IS NULL;

-- Update any NULL workout_end times to use created_at + 30 minutes (reasonable default)
UPDATE workout_data
SET workout_end = created_at + INTERVAL '30 minutes'
WHERE workout_end IS NULL;

-- Now make the columns NOT NULL
ALTER TABLE workout_data
ALTER COLUMN workout_start SET NOT NULL;

ALTER TABLE workout_data
ALTER COLUMN workout_end SET NOT NULL;

-- Add a check constraint to ensure workout_end is after workout_start
ALTER TABLE workout_data
ADD CONSTRAINT workout_times_valid
CHECK (workout_end > workout_start);