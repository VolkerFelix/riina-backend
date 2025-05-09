-- Create user_states table
CREATE TABLE IF NOT EXISTS user_states (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    state_type VARCHAR(50) NOT NULL,  -- e.g., 'fitness', 'sleep', 'stress'
    state_value JSONB NOT NULL,       -- detailed state information
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add indexes
CREATE INDEX IF NOT EXISTS idx_user_states_user_id ON user_states(user_id);
CREATE INDEX IF NOT EXISTS idx_user_states_type ON user_states(state_type);
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_states_user_type ON user_states(user_id, state_type);