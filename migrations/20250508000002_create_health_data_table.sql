-- Create health_data table
CREATE TABLE IF NOT EXISTS health_data (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id VARCHAR(255) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    steps INTEGER,
    heart_rate REAL,
    sleep JSONB,
    active_energy_burned REAL,
    additional_metrics JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for faster lookups
CREATE INDEX IF NOT EXISTS idx_health_data_user_id ON health_data(user_id);
CREATE INDEX IF NOT EXISTS idx_health_data_timestamp ON health_data(timestamp);
CREATE INDEX IF NOT EXISTS idx_health_data_device_id ON health_data(device_id);

-- Create a compound index for user_id and timestamp (common query pattern)
CREATE INDEX IF NOT EXISTS idx_health_data_user_timestamp ON health_data(user_id, timestamp);