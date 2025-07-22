-- Add 'evaluated' status to the league_games status constraint
-- This status indicates that a finished game has been processed and evaluated

-- Drop the existing constraint
ALTER TABLE league_games 
DROP CONSTRAINT IF EXISTS valid_status;

-- Add the updated constraint with 'evaluated' status
ALTER TABLE league_games 
ADD CONSTRAINT valid_status CHECK (status IN ('scheduled', 'in_progress', 'finished', 'evaluated', 'postponed'));