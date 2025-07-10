-- Update week_start_date and week_end_date to TIMESTAMPTZ for proper DateTime handling

-- First, update the data to convert DATE to TIMESTAMPTZ
DO $$ 
BEGIN
    -- Convert week_start_date from DATE to TIMESTAMPTZ (start of day)
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'league_games' 
        AND column_name = 'week_start_date' 
        AND data_type = 'date'
    ) THEN
        ALTER TABLE league_games 
        ALTER COLUMN week_start_date TYPE TIMESTAMPTZ 
        USING week_start_date::timestamptz;
    END IF;
    
    -- Convert week_end_date from DATE to TIMESTAMPTZ (end of day)
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'league_games' 
        AND column_name = 'week_end_date' 
        AND data_type = 'date'
    ) THEN
        ALTER TABLE league_games 
        ALTER COLUMN week_end_date TYPE TIMESTAMPTZ 
        USING (week_end_date::timestamptz + INTERVAL '23 hours 59 minutes');
    END IF;
END $$;

-- Update the constraint to work with TIMESTAMPTZ
DO $$
BEGIN
    -- Drop the old constraint if it exists
    IF EXISTS (
        SELECT 1 FROM information_schema.table_constraints 
        WHERE table_name = 'league_games' 
        AND constraint_name = 'chk_week_dates_order'
    ) THEN
        ALTER TABLE league_games DROP CONSTRAINT chk_week_dates_order;
    END IF;
    
    -- Add the new constraint
    ALTER TABLE league_games 
    ADD CONSTRAINT chk_week_dates_order 
    CHECK (week_end_date >= week_start_date);
END $$;