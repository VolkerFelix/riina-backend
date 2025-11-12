-- Create analytics_events table for storing user analytics data
CREATE TABLE IF NOT EXISTS analytics_events (
    id BIGSERIAL PRIMARY KEY,
    event_name VARCHAR(100) NOT NULL,
    event_data JSONB,
    screen_name VARCHAR(100),
    session_id VARCHAR(100),
    user_hash VARCHAR(16),
    timestamp TIMESTAMPTZ NOT NULL,
    platform VARCHAR(20) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for common query patterns
CREATE INDEX idx_analytics_events_event_name ON analytics_events(event_name);
CREATE INDEX idx_analytics_events_user_hash ON analytics_events(user_hash);
CREATE INDEX idx_analytics_events_session_id ON analytics_events(session_id);
CREATE INDEX idx_analytics_events_timestamp ON analytics_events(timestamp DESC);
CREATE INDEX idx_analytics_events_platform ON analytics_events(platform);

-- Create composite index for common queries (user analytics over time)
CREATE INDEX idx_analytics_events_user_time ON analytics_events(user_hash, timestamp DESC);

-- Create composite index for session analytics
CREATE INDEX idx_analytics_events_session_time ON analytics_events(session_id, timestamp DESC);

-- Add comment to table
COMMENT ON TABLE analytics_events IS 'Stores anonymized user analytics events from the mobile app. user_hash is a SHA-256 hash of the user ID for privacy.';
COMMENT ON COLUMN analytics_events.user_hash IS 'SHA-256 hash (first 16 chars) of user ID for anonymous tracking';
COMMENT ON COLUMN analytics_events.event_data IS 'Structured event data (Session or Screen variant)';
COMMENT ON COLUMN analytics_events.timestamp IS 'Event timestamp from client (milliseconds since epoch)';
