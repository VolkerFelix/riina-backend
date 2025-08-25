-- Fix live_score_events table schema to match code expectations
-- Adds missing columns: event_type and workout_data_id

BEGIN;

-- Check if event_type column exists, if not add it
DO $$ 
BEGIN
    -- Add event_type column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'live_score_events' AND column_name = 'event_type'
    ) THEN
        -- First create the enum type if it doesn't exist
        CREATE TYPE live_score_event_type AS ENUM ('workout_upload', 'power_boost', 'team_bonus', 'milestone');
        
        -- Add the column
        ALTER TABLE live_score_events 
        ADD COLUMN event_type live_score_event_type NOT NULL DEFAULT 'workout_upload';
    END IF;
    
    -- Add workout_data_id column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'live_score_events' AND column_name = 'workout_data_id'
    ) THEN
        ALTER TABLE live_score_events 
        ADD COLUMN workout_data_id UUID REFERENCES workout_data(id);
    END IF;
    
    -- Add description column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'live_score_events' AND column_name = 'description'
    ) THEN
        ALTER TABLE live_score_events 
        ADD COLUMN description TEXT NOT NULL DEFAULT 'Workout upload';
    END IF;
END $$;

COMMIT;