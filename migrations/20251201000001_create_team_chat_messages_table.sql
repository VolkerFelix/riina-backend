-- Create team_chat_messages table
CREATE TABLE team_chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    edited_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT team_chat_messages_message_not_empty CHECK (char_length(message) > 0)
);

-- Create indexes for efficient queries
CREATE INDEX idx_team_chat_messages_team_id ON team_chat_messages(team_id);
CREATE INDEX idx_team_chat_messages_created_at ON team_chat_messages(created_at DESC);
CREATE INDEX idx_team_chat_messages_team_created ON team_chat_messages(team_id, created_at DESC);

-- Add comment
COMMENT ON TABLE team_chat_messages IS 'Stores chat messages for team communication';
