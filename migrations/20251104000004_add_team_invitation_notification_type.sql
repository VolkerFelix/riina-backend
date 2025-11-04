-- Add 'team_invitation' to notification_type CHECK constraint
ALTER TABLE notifications DROP CONSTRAINT IF EXISTS notifications_check_type;
ALTER TABLE notifications ADD CONSTRAINT notifications_check_type
    CHECK (notification_type IN ('reaction', 'comment', 'reply', 'player_pool_event', 'team_invitation'));
