-- Remove the notification_type check constraint to allow flexibility for new notification types
-- The application layer (Rust code) validates notification types via the NotificationType enum
ALTER TABLE notifications DROP CONSTRAINT IF EXISTS notifications_check_type;
