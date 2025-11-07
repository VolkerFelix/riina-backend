-- Add invitation_accepted and invitation_declined notification types
-- Migration: Add notification types for invitation responses
-- Created: 2025-11-04

ALTER TABLE notifications DROP CONSTRAINT IF EXISTS notifications_check_type;

ALTER TABLE notifications ADD CONSTRAINT notifications_check_type
    CHECK (notification_type IN (
        'reaction',
        'comment',
        'reply',
        'player_pool_event',
        'team_invitation',
        'invitation_accepted',
        'invitation_declined'
    ));
