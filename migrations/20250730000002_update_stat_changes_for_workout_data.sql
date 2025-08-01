-- Update stat_changes table to reference workout_data instead of health_data
-- The table was created with a foreign key to health_data_id, now needs to reference workout_data_id

-- Drop the old foreign key constraint
ALTER TABLE stat_changes DROP CONSTRAINT IF EXISTS stat_changes_health_data_id_fkey;

-- Rename the column
ALTER TABLE stat_changes RENAME COLUMN health_data_id TO workout_data_id;

-- Add the new foreign key constraint
ALTER TABLE stat_changes ADD CONSTRAINT stat_changes_workout_data_id_fkey 
    FOREIGN KEY (workout_data_id) REFERENCES workout_data(id) ON DELETE CASCADE;

-- Update the index
DROP INDEX IF EXISTS idx_stat_changes_health_data_id;
CREATE INDEX idx_stat_changes_workout_data_id ON stat_changes(workout_data_id);