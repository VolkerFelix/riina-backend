-- Add workout_data_id column to live_score_events for direct workout linking
ALTER TABLE live_score_events 
ADD COLUMN workout_data_id UUID REFERENCES workout_data(id) ON DELETE CASCADE;

-- Create index for performance
CREATE INDEX idx_live_score_events_workout_data_id ON live_score_events(workout_data_id);