#!/usr/bin/env bash
set -eo pipefail

# Script to set user roles (admin, superadmin, moderator, user)
# Usage: ./scripts/set_user_role.sh <username> <role>
# Example: ./scripts/set_user_role.sh john_doe admin

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Function to show usage
show_usage() {
    echo "Usage: $0 <username> <role>"
    echo ""
    echo "Roles:"
    echo "  user       - Standard user (default)"
    echo "  moderator  - Moderation privileges"
    echo "  admin      - Administrative privileges"
    echo "  superadmin - Highest privilege level"
    echo ""
    echo "Examples:"
    echo "  $0 john_doe admin"
    echo "  $0 jane_smith superadmin"
    echo "  $0 bob_mod moderator"
    echo ""
    exit 1
}

# Check if correct number of arguments provided
if [ $# -ne 2 ]; then
    print_error "Invalid number of arguments"
    show_usage
fi

USERNAME="$1"
ROLE="$2"

# Validate role
case "$ROLE" in
    user|moderator|admin|superadmin)
        ;;
    *)
        print_error "Invalid role: $ROLE"
        echo "Valid roles: user, moderator, admin, superadmin"
        exit 1
        ;;
esac

# Check if required tools are installed
if ! [ -x "$(command -v psql)" ]; then
    print_error "psql is not installed."
    echo "Please install PostgreSQL client tools."
    exit 1
fi

# Load .env file if it exists
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
    print_info "Loaded environment variables from .env"
else
    print_warning ".env file not found, using environment variables"
fi

# Check required environment variables
if [ -z "${POSTGRES__DATABASE__USER}" ] || [ -z "${POSTGRES__DATABASE__PASSWORD}" ]; then
    print_error "Required environment variables not set:"
    echo "  POSTGRES__DATABASE__USER"
    echo "  POSTGRES__DATABASE__PASSWORD"
    echo ""
    echo "Please set these in your .env file or environment."
    exit 1
fi

# Set PostgreSQL password for psql
export PGPASSWORD="${POSTGRES__DATABASE__PASSWORD}"

# Database connection parameters
DB_HOST="${POSTGRES__DATABASE__HOST:-localhost}"
DB_PORT="${POSTGRES__DATABASE__PORT:-5432}"
DB_NAME="${POSTGRES__DATABASE__NAME:-evolveme_db}"
DB_USER="${POSTGRES__DATABASE__USER}"

print_info "Connecting to database: $DB_HOST:$DB_PORT/$DB_NAME"

# Check if database is accessible
if ! psql -h "$DB_HOST" -U "$DB_USER" -p "$DB_PORT" -d "$DB_NAME" -c '\q' >/dev/null 2>&1; then
    print_error "Cannot connect to database"
    echo "Please ensure:"
    echo "  1. PostgreSQL is running"
    echo "  2. Database exists"
    echo "  3. Credentials are correct"
    echo "  4. Network connectivity is available"
    exit 1
fi

print_success "Connected to database"

# Check if user exists
USER_EXISTS=$(psql -h "$DB_HOST" -U "$DB_USER" -p "$DB_PORT" -d "$DB_NAME" -t -c \
    "SELECT COUNT(*) FROM users WHERE username = '$USERNAME';" | tr -d ' ')

if [ "$USER_EXISTS" -eq 0 ]; then
    print_error "User '$USERNAME' does not exist"
    echo ""
    echo "Available users:"
    psql -h "$DB_HOST" -U "$DB_USER" -p "$DB_PORT" -d "$DB_NAME" -c \
        "SELECT username, role, status FROM users ORDER BY username;"
    exit 1
fi

# Get current user info
CURRENT_INFO=$(psql -h "$DB_HOST" -U "$DB_USER" -p "$DB_PORT" -d "$DB_NAME" -t -c \
    "SELECT username, role, status FROM users WHERE username = '$USERNAME';" | tr -s ' ')

print_info "Current user info: $CURRENT_INFO"

# Ask for confirmation unless --yes flag is provided
if [[ "$*" != *"--yes"* ]] && [[ "$*" != *"-y"* ]]; then
    echo ""
    print_warning "Are you sure you want to change the role of '$USERNAME' to '$ROLE'? (y/N)"
    read -r response
    case "$response" in
        [yY][eE][sS]|[yY])
            ;;
        *)
            print_info "Operation cancelled"
            exit 0
            ;;
    esac
fi

# Update user role
print_info "Updating user role..."

UPDATE_RESULT=$(psql -h "$DB_HOST" -U "$DB_USER" -p "$DB_PORT" -d "$DB_NAME" -c \
    "UPDATE users SET role = '$ROLE' WHERE username = '$USERNAME'; SELECT ROW_COUNT();" 2>&1)

if [ $? -eq 0 ]; then
    print_success "Successfully updated user '$USERNAME' to role '$ROLE'"
    
    # Show updated user info
    echo ""
    print_info "Updated user info:"
    psql -h "$DB_HOST" -U "$DB_USER" -p "$DB_PORT" -d "$DB_NAME" -c \
        "SELECT username, role, status, created_at FROM users WHERE username = '$USERNAME';"
    
    echo ""
    print_success "Role change completed!"
    
    # Show additional info based on role
    case "$ROLE" in
        admin|superadmin)
            echo ""
            print_info "Admin privileges info:"
            echo "  • Can access admin panel at /admin routes"
            echo "  • Can manage users, teams, and leagues"
            echo "  • User needs to re-login to get new JWT token with updated role"
            ;;
        moderator)
            echo ""
            print_info "Moderator privileges info:"
            echo "  • Can moderate content and users"
            echo "  • User needs to re-login to get new JWT token with updated role"
            ;;
        user)
            echo ""
            print_info "Standard user privileges"
            echo "  • User needs to re-login to get new JWT token with updated role"
            ;;
    esac
    
else
    print_error "Failed to update user role"
    echo "$UPDATE_RESULT"
    exit 1
fi