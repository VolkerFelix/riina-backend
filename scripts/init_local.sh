#!/usr/bin/env bash
set -eo pipefail

# Detect if we're in CI environment
if [ -n "${CI}" ]; then
    echo "🔧 CI environment detected - using service containers"
    exec "$(dirname "$0")/init_db_and_redis.sh"
fi

echo "🔧 Local development environment detected - using Docker containers"

if ! [ -x "$(command -v psql)" ]; then
    echo >&2 "Error: psql is not installed."
    exit 1
fi
if ! [ -x "$(command -v sqlx)" ]; then
    echo >&2 "Error: sqlx is not installed."
    echo >&2 "Use:"
    echo >&2 " cargo install --version='~0.8' sqlx-cli \
--no-default-features --features rustls,postgres"
    echo >&2 "to install it."
    exit 1
fi
if ! [ -x "$(command -v redis-cli)" ]; then
    echo >&2 "Error: redis-cli is not installed."
    echo >&2 "Use:"
    echo >&2 " brew install redis"
    exit 1
fi
if ! [ -x "$(command -v docker)" ]; then
    echo >&2 "Error: docker is not installed."
    echo >&2 "Please install Docker to run containerized services."
    exit 1
fi

# Load .env file for local development
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
else
    echo ".env file not found."
    exit 1
fi

# Function to check if container is running
container_running() {
    local container_name=$1
    docker ps --format "table {{.Names}}" | grep -q "^${container_name}$"
}

# Function to check if container exists (but may be stopped)
container_exists() {
    local container_name=$1
    docker ps -a --format "table {{.Names}}" | grep -q "^${container_name}$"
}

# Function to wait for service to be ready
wait_for_service() {
    local service_name=$1
    local check_command=$2
    local max_attempts=${3:-30}
    local attempt=1
    
    echo "Waiting for $service_name to be ready..."
    while [ $attempt -le $max_attempts ]; do
        if eval "$check_command" >/dev/null 2>&1; then
            echo "$service_name is ready!"
            return 0
        fi
        echo "Attempt $attempt/$max_attempts: $service_name is not ready yet..."
        sleep 2
        ((attempt++))
    done
    
    echo "Error: $service_name did not become ready within expected time"
    return 1
}

# Clean up postgres, redis and minio containers
clean_up() {
    echo -e "${YELLOW}Cleaning up PostgreSQL, Redis and MinIO containers...${NC}"
    
    # Stop and remove containers if they exist
    if [ "$(docker ps -aq -f name=riina-postgres-test)" ]; then
        docker stop riina-postgres-test 2>/dev/null || true
        docker rm riina-postgres-test 2>/dev/null || true
    fi
    
    if [ "$(docker ps -aq -f name=riina-redis-test)" ]; then
        docker stop riina-redis-test 2>/dev/null || true
        docker rm riina-redis-test 2>/dev/null || true
    fi

    if [ "$(docker ps -aq -f name=riina-minio-test)" ]; then
        docker stop riina-minio-test 2>/dev/null || true
        docker rm riina-minio-test 2>/dev/null || true
    fi

    # Remove volumes if they exist
    if [ "$(docker volume ls -q -f name=riina-postgres-test-data)" ]; then
        docker volume rm riina-postgres-test-data 2>/dev/null || true
    fi
    
    if [ "$(docker volume ls -q -f name=riina-redis-test-data)" ]; then
        docker volume rm riina-redis-test-data 2>/dev/null || true
    fi

    if [ "$(docker volume ls -q -f name=riina-minio-test-data)" ]; then
        docker volume rm riina-minio-test-data 2>/dev/null || true
    fi
}

# Allow to skip Docker if dockerized services are already running
if [[ -z "${SKIP_DOCKER}" ]]
then
    echo "🐳 Starting Docker containers..."

    # Start PostgreSQL container
    echo "Starting PostgreSQL container..."
    if container_running "riina-postgres"; then
        echo "PostgreSQL container already running"
    else
        if container_exists "riina-postgres"; then
            echo "Starting existing PostgreSQL container..."
            docker start riina-postgres
        else
            echo "Creating new PostgreSQL container..."
            docker run \
                --name riina-postgres \
                -e POSTGRES_USER=${POSTGRES__DATABASE__USER} \
                -e POSTGRES_PASSWORD=${POSTGRES__DATABASE__PASSWORD} \
                -e POSTGRES_DB=evolveme_db \
                -p 5432:5432 \
                -d postgres \
                postgres -N 1000
        fi
    fi

    # Start Redis container
    echo "Starting Redis container..."
    if container_running "riina-redis"; then
        echo "Redis container already running"
    else
        if container_exists "riina-redis"; then
            echo "Starting existing Redis container..."
            docker start riina-redis
        else
            echo "Creating new Redis container..."
            docker run \
                --name riina-redis \
                -e REDIS_PASSWORD=${REDIS__REDIS__PASSWORD} \
                -p 6379:6379 \
                -d redis \
                redis-server --requirepass ${REDIS__REDIS__PASSWORD}
        fi
    fi

    echo "✅ All containers started"
else
    echo "SKIP_DOCKER is set - assuming services are already running"
fi

# Wait for services to be ready
echo "🔄 Waiting for services to be ready..."

# Wait for PostgreSQL
export PGPASSWORD="${POSTGRES__DATABASE__PASSWORD}"
wait_for_service "PostgreSQL" "psql -h localhost -U ${POSTGRES__DATABASE__USER} -p 5432 -d evolveme_db -c '\q'"

# Wait for Redis
wait_for_service "Redis" "redis-cli -h localhost -p 6379 -a ${REDIS__REDIS__PASSWORD} ping"

echo "✅ All services are ready!"

# Database setup
echo "🗄️ Setting up database..."
DATABASE_URL=postgres://${POSTGRES__DATABASE__USER}:${POSTGRES__DATABASE__PASSWORD}@localhost:5432/evolveme_db
export DATABASE_URL
sqlx database create
sqlx migrate run
cargo sqlx prepare --database-url $DATABASE_URL

echo "✅ Database has been migrated and is ready! Cleaning up now and exiting..."
clean_up

# Print service status
echo ""
echo "All services have been cleaned up."
echo "========================================"