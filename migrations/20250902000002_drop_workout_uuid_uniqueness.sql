-- Drop the uniqueness constraint on workout_uuid
-- This allows multiple records with the same workout_uuid, which is needed for:
-- 1. Different users sharing the same device/app
-- 2. Retries and error recovery scenarios
-- 3. Testing scenarios

-- Drop the unique constraint
ALTER TABLE workout_data 
DROP CONSTRAINT IF EXISTS unique_workout_uuid;

-- Keep the index for performance but make it non-unique
DROP INDEX IF EXISTS idx_workout_data_workout_uuid;
CREATE INDEX idx_workout_data_workout_uuid ON workout_data(workout_uuid);

-- Also create a composite index for queries that filter by both user and workout
CREATE INDEX IF NOT EXISTS idx_workout_data_user_workout ON workout_data(user_id, workout_uuid);