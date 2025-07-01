-- Add evaluation schedule fields to league_seasons table
ALTER TABLE league_seasons
ADD COLUMN evaluation_cron VARCHAR(100),
ADD COLUMN evaluation_timezone VARCHAR(50) DEFAULT 'UTC',
ADD COLUMN auto_evaluation_enabled BOOLEAN DEFAULT true;

-- Create an index for faster lookups when scheduling jobs
CREATE INDEX idx_league_seasons_auto_evaluation ON league_seasons(auto_evaluation_enabled) WHERE auto_evaluation_enabled = true;

-- Update existing seasons to have default evaluation schedule (daily at 09:00 UTC)
UPDATE league_seasons 
SET evaluation_cron = '0 0 9 * * *', 
    evaluation_timezone = 'UTC', 
    auto_evaluation_enabled = true 
WHERE evaluation_cron IS NULL;
