-- Add heart rate zone breakdown data to stat_changes table
ALTER TABLE stat_changes 
ADD COLUMN zone_breakdown JSONB DEFAULT NULL;

-- Add index for querying zone breakdown data
CREATE INDEX idx_stat_changes_zone_breakdown ON stat_changes USING GIN (zone_breakdown);

-- Add comment explaining the new column
COMMENT ON COLUMN stat_changes.zone_breakdown IS 'JSON breakdown of heart rate zones with minutes spent and stats gained per zone';