-- Change stamina_gained and strength_gained columns from INTEGER to REAL (float)
-- This allows for more precise fractional values in workout scoring

ALTER TABLE workout_data 
ALTER COLUMN stamina_gained TYPE REAL,
ALTER COLUMN strength_gained TYPE REAL;

-- Also update live_score_events table to match
ALTER TABLE live_score_events 
ALTER COLUMN stamina_gained TYPE REAL,
ALTER COLUMN strength_gained TYPE REAL,
ALTER COLUMN score_points TYPE REAL;

-- Update user_avatars table to use REAL for stamina and strength
ALTER TABLE user_avatars 
ALTER COLUMN stamina TYPE REAL,
ALTER COLUMN strength TYPE REAL;

-- Update any existing data to ensure compatibility
-- (No data conversion needed since INTEGER can be cast to REAL automatically)
