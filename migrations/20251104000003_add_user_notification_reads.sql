-- Create table to track which users have read broadcast notifications
CREATE TABLE user_notification_reads (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    notification_id UUID NOT NULL REFERENCES notifications(id) ON DELETE CASCADE,
    read_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, notification_id)
);

-- Create index for faster lookups
CREATE INDEX idx_user_notification_reads_user_id ON user_notification_reads(user_id);
CREATE INDEX idx_user_notification_reads_notification_id ON user_notification_reads(notification_id);
