#!/bin/bash

# Script to set resting heart rate to 65 for users where it is NULL
# Usage: ./set_resting_heart_rate_fly.sh

set -e

echo "🚀 Setting default resting heart rate on Fly.io database..."
echo "📋 This will set resting_heart_rate to 65 for users where it is NULL"
echo ""

# Create a temporary SQL file
TEMP_SQL=$(mktemp)
cat > "$TEMP_SQL" << 'EOF'
-- Set resting heart rate to 65 for users where it is NULL

UPDATE user_health_profiles
SET
    resting_heart_rate = 65,
    last_updated = NOW()
WHERE resting_heart_rate IS NULL;

-- Show summary of updates
SELECT
    'Resting heart rate update completed' as status,
    COUNT(*) as users_updated
FROM user_health_profiles
WHERE resting_heart_rate = 65
    AND last_updated > NOW() - INTERVAL '1 minute';

-- Show overall statistics
SELECT
    'Overall statistics:' as info,
    COUNT(*) as total_users,
    COUNT(CASE WHEN resting_heart_rate IS NOT NULL THEN 1 END) as users_with_resting_hr,
    COUNT(CASE WHEN resting_heart_rate IS NULL THEN 1 END) as users_without_resting_hr,
    AVG(resting_heart_rate)::INTEGER as avg_resting_hr,
    MIN(resting_heart_rate) as min_resting_hr,
    MAX(resting_heart_rate) as max_resting_hr
FROM user_health_profiles;
EOF

# Execute the SQL commands
echo "⚡ Updating resting heart rates..."
echo ""

fly postgres connect -a evolveme-db -d evolveme_db < "$TEMP_SQL"

# Clean up
rm -f "$TEMP_SQL"

echo ""
echo "✅ Resting heart rate update completed!"
echo "💡 All users with NULL resting_heart_rate now have it set to 65 bpm"
echo ""
