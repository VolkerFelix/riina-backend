-- Mark all existing messages as read for all team members
-- This is a one-time migration to initialize the message_read_status table
-- with the assumption that all messages sent before this migration are already read

INSERT INTO message_read_status (user_id, message_id, read_at)
SELECT DISTINCT tm.user_id, tcm.id, tcm.created_at
FROM team_chat_messages tcm
INNER JOIN team_members tm ON tm.team_id = tcm.team_id
WHERE tm.user_id != tcm.user_id  -- Don't mark own messages as read
  AND tcm.deleted_at IS NULL
  AND tm.status = 'active'
ON CONFLICT (user_id, message_id) DO NOTHING;
