#!/bin/bash

# Script to clean up player pool on Fly.io PostgreSQL database
# Removes users from player_pool who are already on a team
# Usage: ./cleanup_player_pool_fly.sh

set -e

echo "🚀 Cleaning up player pool on Fly.io database..."
echo "📋 This will remove users from player_pool who are already on a team"
echo ""

# Create a temporary SQL file
TEMP_SQL=$(mktemp)
cat > "$TEMP_SQL" << 'EOF'
-- Show current state before cleanup
SELECT
    'Before cleanup' as status,
    COUNT(*) as player_pool_count,
    (SELECT COUNT(DISTINCT user_id) FROM team_members WHERE status = 'active') as users_on_teams,
    (SELECT COUNT(*) FROM player_pool pp
     WHERE EXISTS (
         SELECT 1 FROM team_members tm
         WHERE tm.user_id = pp.user_id AND tm.status = 'active'
     )) as invalid_pool_entries
FROM player_pool;

-- Remove users from player pool who are on teams
DELETE FROM player_pool
WHERE user_id IN (
    SELECT DISTINCT user_id
    FROM team_members
    WHERE status = 'active'
);

-- Show results after cleanup
SELECT
    'After cleanup' as status,
    COUNT(*) as player_pool_count,
    (SELECT COUNT(DISTINCT user_id) FROM team_members WHERE status = 'active') as users_on_teams,
    (SELECT COUNT(*) FROM player_pool pp
     WHERE EXISTS (
         SELECT 1 FROM team_members tm
         WHERE tm.user_id = pp.user_id AND tm.status = 'active'
     )) as invalid_pool_entries
FROM player_pool;

-- Show some examples of remaining player pool users
SELECT
    u.username,
    u.email,
    u.status as user_status,
    pp.last_active_at,
    CASE
        WHEN EXISTS (SELECT 1 FROM team_members tm WHERE tm.user_id = u.id)
        THEN 'Was on team (inactive member)'
        ELSE 'Never on team'
    END as team_history
FROM player_pool pp
JOIN users u ON pp.user_id = u.id
ORDER BY pp.last_active_at DESC
LIMIT 10;
EOF

# Execute the SQL commands
echo "⚡ Running cleanup query..."
echo ""

fly postgres connect -a evolveme-db -d evolveme_db < "$TEMP_SQL"

# Clean up
rm -f "$TEMP_SQL"

echo ""
echo "✅ Player pool cleanup completed!"
echo "💡 All users who are active members of teams have been removed from the player pool"
