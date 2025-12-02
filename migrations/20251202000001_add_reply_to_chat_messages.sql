-- Add reply_to_message_id column to support message replies
ALTER TABLE team_chat_messages
ADD COLUMN reply_to_message_id UUID REFERENCES team_chat_messages(id) ON DELETE SET NULL;

-- Create index for efficient reply lookups
CREATE INDEX idx_team_chat_messages_reply_to ON team_chat_messages(reply_to_message_id);

-- Add comment
COMMENT ON COLUMN team_chat_messages.reply_to_message_id IS 'References the message being replied to';
