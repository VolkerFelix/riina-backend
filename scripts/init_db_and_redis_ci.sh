#!/usr/bin/env bash
set -eo pipefail

echo "🔧 CI Database and Redis initialization script"

# Check if we're in CI environment
if [ -n "${CI}" ]; then
    echo "✅ CI environment detected"
else
    echo "⚠️  Warning: This script is designed for CI environments"
fi

# Check required commands are available
if ! [ -x "$(command -v psql)" ]; then
    echo >&2 "Error: psql is not installed."
    exit 1
fi

if ! [ -x "$(command -v sqlx)" ]; then
    echo >&2 "Error: sqlx is not installed."
    exit 1
fi

if ! [ -x "$(command -v redis-cli)" ]; then
    echo >&2 "Error: redis-cli is not installed."
    exit 1
fi

# Load environment variables (they should already be set in CI)
POSTGRES_USER=${POSTGRES__DATABASE__USER}
POSTGRES_PASSWORD=${POSTGRES__DATABASE__PASSWORD}
POSTGRES_HOST=localhost
POSTGRES_PORT=5432
POSTGRES_DB=evolveme_db

REDIS_HOST=localhost
REDIS_PORT=6379
REDIS_PASSWORD=${REDIS__REDIS__PASSWORD}

APP_USER=${APP__APPLICATION__USER}
APP_PASSWORD=${APP__APPLICATION__PASSWORD}

# Validate required environment variables
if [ -z "$POSTGRES_PASSWORD" ] || [ -z "$REDIS_PASSWORD" ] || [ -z "$APP_USER" ] || [ -z "$APP_PASSWORD" ]; then
    echo "Error: Missing required environment variables"
    echo "Required: POSTGRES__DATABASE__PASSWORD, REDIS__REDIS__PASSWORD, APP__APPLICATION__USER, APP__APPLICATION__PASSWORD"
    exit 1
fi

# Function to wait for service to be ready
wait_for_service() {
    local service_name=$1
    local check_command=$2
    local max_attempts=${3:-30}
    local attempt=1
    
    echo "Waiting for $service_name to be ready..."
    while [ $attempt -le $max_attempts ]; do
        if eval "$check_command" >/dev/null 2>&1; then
            echo "✅ $service_name is ready!"
            return 0
        fi
        echo "Attempt $attempt/$max_attempts: $service_name is not ready yet..."
        sleep 2
        ((attempt++))
    done
    
    echo "❌ Error: $service_name did not become ready within expected time"
    return 1
}

# Wait for PostgreSQL to be ready
echo "🗄️ Checking PostgreSQL connection..."
export PGPASSWORD="$POSTGRES_PASSWORD"
wait_for_service "PostgreSQL" "pg_isready -h $POSTGRES_HOST -p $POSTGRES_PORT -U $POSTGRES_USER"

# Wait for Redis to be ready
echo "🔴 Checking Redis connection..."
wait_for_service "Redis" "redis-cli -h $REDIS_HOST -p $REDIS_PORT -a $REDIS_PASSWORD ping"

# Set up DATABASE_URL
DATABASE_URL="postgres://$POSTGRES_USER:$POSTGRES_PASSWORD@$POSTGRES_HOST:$POSTGRES_PORT/$POSTGRES_DB"
export DATABASE_URL

echo "🗄️ Setting up database..."

# Create application user if it doesn't exist
echo "Creating application user..."
CREATE_USER_QUERY="DO \$\$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = '$APP_USER') THEN
        CREATE USER $APP_USER WITH PASSWORD '$APP_PASSWORD';
    END IF;
END
\$\$;"

PGPASSWORD="$POSTGRES_PASSWORD" psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "$CREATE_USER_QUERY"

# Grant necessary privileges
echo "Granting privileges to application user..."
GRANT_QUERY="ALTER USER $APP_USER CREATEDB;"
PGPASSWORD="$POSTGRES_PASSWORD" psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "$GRANT_QUERY"

# Create database if it doesn't exist (usually already created by service)
echo "Ensuring database exists..."
sqlx database create --database-url "$DATABASE_URL" 2>/dev/null || echo "Database already exists"

# Run migrations
echo "Running database migrations..."
sqlx migrate run --database-url "$DATABASE_URL"

# Test database connection
echo "🧪 Testing database connection..."
PGPASSWORD="$POSTGRES_PASSWORD" psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "SELECT 1;" >/dev/null

# Test Redis connection
echo "🧪 Testing Redis connection..."
redis_test_result=$(redis-cli -h "$REDIS_HOST" -p "$REDIS_PORT" -a "$REDIS_PASSWORD" ping)
if [ "$redis_test_result" != "PONG" ]; then
    echo "❌ Redis connection test failed"
    exit 1
fi

echo ""
echo "🎉 Database and Redis are ready for CI!"
echo "========================================"
echo "PostgreSQL: $POSTGRES_HOST:$POSTGRES_PORT/$POSTGRES_DB"
echo "Redis: $REDIS_HOST:$REDIS_PORT"
echo "Database URL: postgres://$POSTGRES_USER:***@$POSTGRES_HOST:$POSTGRES_PORT/$POSTGRES_DB"
echo ""