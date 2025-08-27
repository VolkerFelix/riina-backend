-- Make home_score and away_score non-nullable with default 0
-- Games always have scores (starting at 0-0)

BEGIN;

-- First, update any NULL values to 0
UPDATE games 
SET home_score = 0 
WHERE home_score IS NULL;

UPDATE games 
SET away_score = 0 
WHERE away_score IS NULL;

-- Now make the columns NOT NULL with DEFAULT 0
ALTER TABLE games 
    ALTER COLUMN home_score SET NOT NULL,
    ALTER COLUMN home_score SET DEFAULT 0;

ALTER TABLE games 
    ALTER COLUMN away_score SET NOT NULL,
    ALTER COLUMN away_score SET DEFAULT 0;

COMMIT;