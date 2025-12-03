-- Add gif_url column to team_chat_messages table
ALTER TABLE team_chat_messages
ADD COLUMN gif_url TEXT NULL;

-- Add comment
COMMENT ON COLUMN team_chat_messages.gif_url IS 'URL of GIF attached to the message (from Tenor API)';
