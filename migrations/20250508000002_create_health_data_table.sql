-- Create health_data table
CREATE TABLE IF NOT EXISTS health_data (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id VARCHAR(255) NOT NULL,
    heart_rate_data JSONB NOT NULL, -- Time series: [{"timestamp": "2024-01-01T10:00:00Z", "hr": 120}, ...]
    active_energy_burned REAL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for faster lookups
CREATE INDEX IF NOT EXISTS idx_health_data_user_id ON health_data(user_id);
CREATE INDEX IF NOT EXISTS idx_health_data_device_id ON health_data(device_id);