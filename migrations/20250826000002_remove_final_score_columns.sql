-- Remove redundant final score columns - use home_score and away_score instead

BEGIN;

-- The final score columns are redundant with the live scoring columns
-- home_score and away_score already contain the final scores
ALTER TABLE games DROP COLUMN IF EXISTS home_score_final;
ALTER TABLE games DROP COLUMN IF EXISTS away_score_final;

COMMIT;