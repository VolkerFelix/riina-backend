-- Create stat_changes table to track game stats gained from each workout
CREATE TABLE stat_changes (
    id UUID NOT NULL PRIMARY KEY DEFAULT gen_random_uuid(),
    health_data_id UUID NOT NULL REFERENCES health_data(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    stamina_change INTEGER NOT NULL DEFAULT 0,
    strength_change INTEGER NOT NULL DEFAULT 0,
    reasoning TEXT[] DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Ensure one stat change record per health data entry
    CONSTRAINT unique_health_data_stat UNIQUE (health_data_id)
);

-- Create indexes for performance
CREATE INDEX idx_stat_changes_user_id ON stat_changes(user_id);
CREATE INDEX idx_stat_changes_health_data_id ON stat_changes(health_data_id);
CREATE INDEX idx_stat_changes_created_at ON stat_changes(created_at DESC);

-- Add a comment explaining the table
COMMENT ON TABLE stat_changes IS 'Tracks game stat changes (stamina/strength) gained from each workout/health data upload';