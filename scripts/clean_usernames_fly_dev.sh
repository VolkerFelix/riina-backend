#!/bin/bash

# Script to clean usernames in Fly.io PostgreSQL database
# - Trims trailing/leading spaces
# - Replaces middle spaces with underscores
# Usage: ./clean_usernames_fly.sh [--dry-run]
# Example: ./clean_usernames_fly.sh --dry-run  (preview changes)
#          ./clean_usernames_fly.sh             (apply changes)

set -e

DRY_RUN=false
if [ "$1" = "--dry-run" ]; then
    DRY_RUN=true
    echo "🔍 DRY RUN MODE - No changes will be made"
    echo ""
fi

echo "🚀 Cleaning usernames in Fly.io database..."
echo ""

# Create a temporary SQL file
TEMP_SQL=$(mktemp)

if [ "$DRY_RUN" = true ]; then
    # Dry run: Just show what would be changed
    cat > "$TEMP_SQL" << 'EOF'
-- Find usernames with spaces (leading, trailing, or middle)
SELECT
    username as current_username,
    REPLACE(TRIM(username), ' ', '_') as new_username,
    CASE
        WHEN username != REPLACE(TRIM(username), ' ', '_') THEN 'WILL CHANGE'
        ELSE 'OK'
    END as status,
    LENGTH(username) - LENGTH(TRIM(username)) as spaces_trimmed,
    LENGTH(username) - LENGTH(REPLACE(username, ' ', '')) as total_spaces
FROM users
WHERE username != REPLACE(TRIM(username), ' ', '_')
ORDER BY username;

-- Count affected users
SELECT
    COUNT(*) as affected_users,
    SUM(CASE WHEN username LIKE ' %' OR username LIKE '% ' THEN 1 ELSE 0 END) as with_trailing_spaces,
    SUM(CASE WHEN username LIKE '% %' THEN 1 ELSE 0 END) as with_middle_spaces
FROM users
WHERE username != REPLACE(TRIM(username), ' ', '_');
EOF

    echo "📊 Preview of changes:"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
else
    # Check for conflicts first
    cat > "$TEMP_SQL" << 'EOF'
-- STEP 1: Check for potential username conflicts
WITH cleaned_usernames AS (
    SELECT
        username as original,
        REPLACE(TRIM(username), ' ', '_') as cleaned,
        id
    FROM users
    WHERE username != REPLACE(TRIM(username), ' ', '_')
),
conflict_check AS (
    SELECT
        c.cleaned,
        COUNT(*) as conflict_count
    FROM cleaned_usernames c
    GROUP BY c.cleaned
    HAVING COUNT(*) > 1

    UNION

    SELECT
        c.cleaned,
        COUNT(*) as conflict_count
    FROM cleaned_usernames c
    INNER JOIN users u ON u.username = c.cleaned AND u.id != c.id
    GROUP BY c.cleaned
)
SELECT * FROM conflict_check;

-- STEP 2: If no conflicts shown above, we proceed with updates
-- Update usernames by trimming and replacing spaces with underscores
UPDATE users
SET username = REPLACE(TRIM(username), ' ', '_')
WHERE username != REPLACE(TRIM(username), ' ', '_');

-- STEP 3: Show updated usernames
SELECT
    username,
    email,
    created_at,
    updated_at
FROM users
WHERE username LIKE '%_%'
ORDER BY updated_at DESC
LIMIT 20;

-- STEP 4: Summary statistics
SELECT
    COUNT(*) as total_users_updated
FROM users
WHERE updated_at >= NOW() - INTERVAL '1 minute';
EOF

    echo "⚡ Checking for conflicts and updating usernames..."
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo "⚠️  IMPORTANT: If conflicts are detected, the script will show them"
    echo "   and you should resolve them manually before proceeding."
    echo ""
    read -p "Press ENTER to continue or Ctrl+C to cancel..."
    echo ""
fi

# Execute the SQL commands on Fly.io
fly postgres connect -a evolveme-db-dev -d evolveme_db < "$TEMP_SQL"

# Clean up
rm -f "$TEMP_SQL"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [ "$DRY_RUN" = true ]; then
    echo "✅ Dry run completed!"
    echo ""
    echo "💡 Next steps:"
    echo "   1. Review the changes above"
    echo "   2. If everything looks good, run: ./clean_usernames_fly.sh"
    echo "   3. Inform affected users they may need to re-login"
else
    echo "✅ Username cleanup completed!"
    echo ""
    echo "⚠️  Important notes:"
    echo "   - Users with updated usernames will need to re-login"
    echo "   - Their JWT tokens are now invalid (username mismatch)"
    echo "   - Old usernames in historical data (game summaries, etc.) are unchanged"
    echo ""
    echo "📱 Recommended: Notify affected users about their new username format"
fi

echo ""
