-- Create message_read_status table to track which messages users have read
CREATE TABLE message_read_status (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message_id UUID NOT NULL REFERENCES team_chat_messages(id) ON DELETE CASCADE,
    read_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, message_id)
);

-- Create indexes for efficient queries
CREATE INDEX idx_message_read_status_user ON message_read_status(user_id);
CREATE INDEX idx_message_read_status_message ON message_read_status(message_id);

-- Add comment
COMMENT ON TABLE message_read_status IS 'Tracks which messages each user has read';
