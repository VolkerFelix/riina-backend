#!/bin/bash

# Script to set user role on Fly.io PostgreSQL database
# Usage: ./set_user_role_fly_v2.sh <username> <role>
# Example: ./set_user_role_fly_v2.sh admin superadmin

set -e

# Check if correct number of arguments provided
if [ $# -ne 2 ]; then
    echo "❌ Error: Incorrect number of arguments"
    echo "Usage: $0 <username> <role>"
    echo "Example: $0 admin superadmin"
    echo ""
    echo "Available roles:"
    echo "  - superadmin: Full system access"
    echo "  - admin: Administrative access"
    echo "  - moderator: Moderation capabilities"
    echo "  - user: Regular user (default)"
    exit 1
fi

USERNAME=$1
ROLE=$2

# Validate role
VALID_ROLES=("superadmin" "admin" "moderator" "user")
if [[ ! " ${VALID_ROLES[@]} " =~ " ${ROLE} " ]]; then
    echo "❌ Error: Invalid role '${ROLE}'"
    echo "Valid roles are: ${VALID_ROLES[@]}"
    exit 1
fi

echo "🚀 Setting user role on Fly.io database..."
echo "📋 User: ${USERNAME}"
echo "🔑 Role: ${ROLE}"

# Create a temporary SQL file
TEMP_SQL=$(mktemp)
cat > "$TEMP_SQL" << EOF
-- Update user role
UPDATE users SET role = '${ROLE}' WHERE username = '${USERNAME}';

-- Show updated user
SELECT username, email, role, status, created_at 
FROM users 
WHERE username = '${USERNAME}';
EOF

# Execute the SQL commands
echo ""
echo "⚡ Updating user role..."

fly postgres connect -a evolveme-db -d evolveme_db < "$TEMP_SQL"

# Clean up
rm -f "$TEMP_SQL"

echo ""
echo "✅ User role update completed!"
echo "💡 The user ${USERNAME} now has ${ROLE} privileges"