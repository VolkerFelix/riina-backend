-- Create push_tokens table for storing user device push notification tokens
CREATE TABLE push_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token TEXT NOT NULL,
    platform TEXT NOT NULL CHECK (platform IN ('ios', 'android', 'web')),
    device_info JSONB,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    UNIQUE(token)
);

-- Create index on user_id for faster lookups
CREATE INDEX idx_push_tokens_user_id ON push_tokens(user_id);

-- Create index on active tokens for efficient filtering
CREATE INDEX idx_push_tokens_active ON push_tokens(user_id, is_active) WHERE is_active = true;

-- Create updated_at trigger
CREATE OR REPLACE FUNCTION update_push_tokens_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER push_tokens_updated_at
    BEFORE UPDATE ON push_tokens
    FOR EACH ROW
    EXECUTE FUNCTION update_push_tokens_updated_at();
