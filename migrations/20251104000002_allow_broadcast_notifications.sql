-- Allow broadcast notifications (for player pool events)
-- These notifications don't have a specific recipient - they're shown to all users

-- 1. Make recipient_id nullable to allow broadcast notifications
ALTER TABLE notifications ALTER COLUMN recipient_id DROP NOT NULL;

-- 2. Update the CHECK constraint to include 'player_pool_event'
ALTER TABLE notifications DROP CONSTRAINT IF EXISTS notifications_check_type;
ALTER TABLE notifications ADD CONSTRAINT notifications_check_type
    CHECK (notification_type IN ('reaction', 'comment', 'reply', 'player_pool_event'));

-- 3. Add index for broadcast notifications (where recipient_id IS NULL)
CREATE INDEX idx_notifications_broadcast ON notifications(created_at DESC)
    WHERE recipient_id IS NULL;
