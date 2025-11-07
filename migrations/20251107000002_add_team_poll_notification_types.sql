-- Add team poll notification types
-- Migration: Add notification types for team polls
-- Created: 2025-11-07

ALTER TABLE notifications DROP CONSTRAINT IF EXISTS notifications_check_type;

ALTER TABLE notifications ADD CONSTRAINT notifications_check_type
    CHECK (notification_type IN (
        'reaction',
        'comment',
        'reply',
        'player_pool_event',
        'team_invitation',
        'invitation_accepted',
        'invitation_declined',
        'team_poll',
        'team_poll_created',
        'team_poll_completed',
        'team_poll_expired',
        'removed_from_team'
    ));
