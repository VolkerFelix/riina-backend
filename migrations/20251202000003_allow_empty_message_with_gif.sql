-- Drop the old constraint that requires message to not be empty
ALTER TABLE team_chat_messages
DROP CONSTRAINT IF EXISTS team_chat_messages_message_not_empty;

-- Add new constraint: message must not be empty OR gif_url must be present
ALTER TABLE team_chat_messages
ADD CONSTRAINT team_chat_messages_message_or_gif CHECK (
    (message IS NOT NULL AND LENGTH(TRIM(message)) > 0) OR
    (gif_url IS NOT NULL AND LENGTH(TRIM(gif_url)) > 0)
);

COMMENT ON CONSTRAINT team_chat_messages_message_or_gif ON team_chat_messages IS
'Ensures that either message text or a GIF URL is present (or both)';
