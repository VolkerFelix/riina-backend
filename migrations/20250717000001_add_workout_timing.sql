-- Add workout timing fields to health_data table
ALTER TABLE health_data 
ADD COLUMN workout_start TIMESTAMPTZ,
ADD COLUMN workout_end TIMESTAMPTZ;

-- Create index for workout_start for efficient queries in Grafana
CREATE INDEX IF NOT EXISTS idx_health_data_workout_start ON health_data(workout_start);