-- Remove redundant stat_changes table since all data is now stored in workout_data

-- First ensure workout_data has all necessary columns (if not already there)
ALTER TABLE workout_data 
ADD COLUMN IF NOT EXISTS stamina_gained INTEGER DEFAULT 0,
ADD COLUMN IF NOT EXISTS strength_gained INTEGER DEFAULT 0,
ADD COLUMN IF NOT EXISTS total_points_gained INTEGER DEFAULT 0;

-- Drop the stat_changes table
DROP TABLE IF EXISTS stat_changes;

-- Remove any indexes that were on stat_changes (they're dropped with the table, but being explicit)
-- The indexes were: idx_stat_changes_user_id, idx_stat_changes_health_data_id, idx_stat_changes_created_at