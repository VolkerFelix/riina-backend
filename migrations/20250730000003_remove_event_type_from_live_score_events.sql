-- Remove event_type column from live_score_events table as it's not needed
ALTER TABLE live_score_events DROP COLUMN event_type;

-- Drop the enum type as it's no longer used
DROP TYPE live_score_event_type;