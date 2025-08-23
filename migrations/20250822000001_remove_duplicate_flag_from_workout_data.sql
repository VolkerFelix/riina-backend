-- Remove is_duplicate flag from workout_data table
-- We now reject duplicate workouts at upload time instead of storing them

-- Drop the index first
DROP INDEX IF EXISTS idx_workout_data_is_duplicate;

-- Remove the column
ALTER TABLE workout_data 
DROP COLUMN IF EXISTS is_duplicate;