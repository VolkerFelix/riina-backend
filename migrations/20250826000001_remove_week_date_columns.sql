-- Remove redundant date/time columns
-- week_start_date/week_end_date are duplicates of game_start_time/game_end_time
-- scheduled_time is also a duplicate of game_start_time

-- First, copy any existing data to game_* fields where game_* is null
UPDATE games 
SET game_start_time = COALESCE(game_start_time, week_start_date, scheduled_time),
    game_end_time = COALESCE(game_end_time, week_end_date)
WHERE game_start_time IS NULL OR game_end_time IS NULL;

-- Now drop the redundant columns
ALTER TABLE games 
DROP COLUMN IF EXISTS week_start_date,
DROP COLUMN IF EXISTS week_end_date,
DROP COLUMN IF EXISTS scheduled_time;