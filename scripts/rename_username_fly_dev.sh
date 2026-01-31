#!/bin/bash

# Script to rename a username in Fly.io PostgreSQL database
# Usage: ./rename_username_fly.sh <old_username> <new_username>
# Example: ./rename_username_fly.sh "Robert K" "Robert_K"

set -e

# Check if correct number of arguments provided
if [ $# -ne 2 ]; then
    echo "❌ Error: Incorrect number of arguments"
    echo "Usage: $0 <old_username> <new_username>"
    echo "Example: $0 \"Robert K\" \"Robert_K\""
    echo ""
    echo "💡 Tips:"
    echo "  - Use quotes if username contains spaces"
    echo "  - New username should not contain spaces for @mention functionality"
    echo "  - User will need to re-login with new username"
    exit 1
fi

OLD_USERNAME=$1
NEW_USERNAME=$2

# Validate new username (no leading/trailing spaces, warn about middle spaces)
if [[ "$NEW_USERNAME" =~ ^[[:space:]] || "$NEW_USERNAME" =~ [[:space:]]$ ]]; then
    echo "❌ Error: New username cannot have leading or trailing spaces"
    exit 1
fi

if [[ "$NEW_USERNAME" =~ [[:space:]] ]]; then
    echo "⚠️  Warning: New username contains spaces"
    echo "   For @mention functionality, consider using underscores instead"
    echo "   Example: \"${NEW_USERNAME// /_}\""
    read -p "Continue anyway? (y/N): " confirm
    if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
        echo "Cancelled."
        exit 0
    fi
fi

echo "🚀 Renaming username in Fly.io database..."
echo "📋 Old username: \"${OLD_USERNAME}\""
echo "📋 New username: \"${NEW_USERNAME}\""
echo ""

# Create a temporary SQL file
TEMP_SQL=$(mktemp)
cat > "$TEMP_SQL" << EOF
-- STEP 1: Check if old username exists
SELECT
    CASE
        WHEN COUNT(*) = 0 THEN 'ERROR: Old username not found'
        ELSE 'OK: Old username exists'
    END as check_old_username,
    COUNT(*) as count
FROM users
WHERE username = '${OLD_USERNAME}';

-- STEP 2: Check if new username already exists (conflict check)
SELECT
    CASE
        WHEN COUNT(*) > 0 THEN 'ERROR: New username already taken'
        ELSE 'OK: New username available'
    END as check_new_username,
    COUNT(*) as count
FROM users
WHERE username = '${NEW_USERNAME}';

-- STEP 3: Show user details before update
SELECT
    id,
    username,
    email,
    role,
    status,
    created_at,
    updated_at
FROM users
WHERE username = '${OLD_USERNAME}';

-- STEP 4: Perform the update
UPDATE users
SET username = '${NEW_USERNAME}', updated_at = NOW()
WHERE username = '${OLD_USERNAME}';

-- STEP 5: Show updated user
SELECT
    id,
    username,
    email,
    role,
    status,
    created_at,
    updated_at
FROM users
WHERE username = '${NEW_USERNAME}';

-- STEP 6: Summary
SELECT
    CASE
        WHEN COUNT(*) = 1 THEN 'SUCCESS: Username renamed'
        ELSE 'ERROR: Update failed'
    END as result
FROM users
WHERE username = '${NEW_USERNAME}';
EOF

echo "⚡ Executing username update..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Execute the SQL commands
fly postgres connect -a evolveme-db-dev -d evolveme_db < "$TEMP_SQL"

# Clean up
rm -f "$TEMP_SQL"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ Username rename completed!"
echo ""
echo "⚠️  Important notes:"
echo "   - User's JWT token is now invalid (contains old username)"
echo "   - User MUST re-login with: \"${NEW_USERNAME}\""
echo "   - Historical data (game summaries, events) still shows old username"
echo ""
echo "📱 Next steps:"
echo "   1. Notify the user about their new username"
echo "   2. Ask them to re-login using: \"${NEW_USERNAME}\""
echo "   3. Their password remains the same"
echo ""
