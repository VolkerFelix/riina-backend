-- Migration: Add all active users without a team to the player pool
-- This ensures they appear in the leaderboard as free agents

INSERT INTO player_pool (user_id, last_active_at)
SELECT
    u.id,
    NOW()
FROM users u
WHERE
    -- User is active
    u.status = 'active'
    -- User is not in any active team
    AND NOT EXISTS (
        SELECT 1
        FROM team_members tm
        WHERE tm.user_id = u.id
        AND tm.status = 'active'
    )
    -- User is not already in player pool
    AND NOT EXISTS (
        SELECT 1
        FROM player_pool pp
        WHERE pp.user_id = u.id
    )
ON CONFLICT (user_id) DO NOTHING;
