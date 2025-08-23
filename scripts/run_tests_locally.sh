#!/bin/bash
set -euo pipefail

# Color codes for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Load .env file
PARENT_DIR="$(dirname "$(pwd)")"
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
else
    echo ".env file not found."
    exit 1
fi

export APP_ENVIRONMENT="test"
# PostgreSQL Configuration
export POSTGRES__DATABASE__USER=${POSTGRES__DATABASE__USER:-postgres}
export POSTGRES__DATABASE__PASSWORD=${POSTGRES__DATABASE__PASSWORD:-postgres}
# Actix Web Configuration
export APP__APPLICATION__USER=${APP__APPLICATION__USER:-testuser}
export APP__APPLICATION__PASSWORD=${APP__APPLICATION__PASSWORD:-testpassword}
# Redis Config
export REDIS__REDIS__PASSWORD=${REDIS__REDIS__PASSWORD:-redis}
# MinIO Configuration
export MINIO__MINIO__ACCESS_KEY=${MINIO__MINIO__ACCESS_KEY:-minioadmin}
export MINIO__MINIO__SECRET_KEY=${MINIO__MINIO__SECRET_KEY:-minioadmin}

# Default configuration
DB_HOST="localhost"
DB_PORT=5432
DB_USER=${POSTGRES__DATABASE__USER}
DB_NAME="evolveme_db_test"
DB_PASSWORD=${POSTGRES__DATABASE__PASSWORD}

# Explicitly set DATABASE_URL for SQLx
export DATABASE_URL="postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME"

# Check if PostgreSQL container is already running
check_postgres() {
    if [ "$(docker ps -q -f name=evolveme-postgres-test)" ]; then
        echo -e "${GREEN}PostgreSQL container is already running.${NC}"
    else
        echo -e "${RED}PostgreSQL container is not running.${NC}"
        spin_up_postgres
    fi
}

# Check if Redis container is already running
check_redis() {
    if [ "$(docker ps -q -f name=evolveme-redis-test)" ]; then
        echo -e "${GREEN}Redis container is already running.${NC}"
    else
        echo -e "${RED}Redis container is not running.${NC}"
        spin_up_redis
    fi
}

# Check if MinIO container is already running

check_minio() {
    if [ "$(docker ps -q -f name=evolveme-minio-test)" ]; then
        echo -e "${GREEN}MinIO container is already running.${NC}"
    else
        echo -e "${RED}MinIO container is not running.${NC}"
        spin_up_minio
    fi
}

# Spin up postgres container
spin_up_postgres() {
    echo -e "${YELLOW}Spinning up PostgreSQL container for tests...${NC}"
    docker run --name evolveme-postgres-test \
        -e POSTGRES_USER=${POSTGRES__DATABASE__USER} \
        -e POSTGRES_PASSWORD=${POSTGRES__DATABASE__PASSWORD} \
        -e POSTGRES_DB=${DB_NAME} \
        -v evolveme-postgres-test-data:/var/lib/postgresql/data \
        -p ${DB_PORT}:5432 \
        -d postgres
}

# Spin up redis container
spin_up_redis() {
    echo -e "${YELLOW}Spinning up Redis container for tests...${NC}"
    docker run --name evolveme-redis-test \
        -e REDIS_PASSWORD=${REDIS__REDIS__PASSWORD} \
        -v evolveme-redis-test-data:/data \
        -p 6379:6379 \
        -d redis \
        redis-server --requirepass ${REDIS__REDIS__PASSWORD}
}

# Spin up minio container

spin_up_minio() {
    echo -e "${YELLOW}Spinning up MinIO container for tests...${NC}"
    docker run --name evolveme-minio-test \
        -e MINIO_ACCESS_KEY=${MINIO__MINIO__ACCESS_KEY} \
        -e MINIO_SECRET_KEY=${MINIO__MINIO__SECRET_KEY} \
        -v evolveme-minio-test-data:/data \
        -p 9000:9000 \
        -d minio/minio server /data
}

# Clean up postgres, redis and minio containers
clean_up() {
    echo -e "${YELLOW}Cleaning up PostgreSQL, Redis and MinIO containers...${NC}"
    
    # Stop and remove containers if they exist
    if [ "$(docker ps -aq -f name=evolveme-postgres-test)" ]; then
        docker stop evolveme-postgres-test 2>/dev/null || true
        docker rm evolveme-postgres-test 2>/dev/null || true
    fi
    
    if [ "$(docker ps -aq -f name=evolveme-redis-test)" ]; then
        docker stop evolveme-redis-test 2>/dev/null || true
        docker rm evolveme-redis-test 2>/dev/null || true
    fi

    if [ "$(docker ps -aq -f name=evolveme-minio-test)" ]; then
        docker stop evolveme-minio-test 2>/dev/null || true
        docker rm evolveme-minio-test 2>/dev/null || true
    fi

    # Remove volumes if they exist
    if [ "$(docker volume ls -q -f name=evolveme-postgres-test-data)" ]; then
        docker volume rm evolveme-postgres-test-data 2>/dev/null || true
    fi
    
    if [ "$(docker volume ls -q -f name=evolveme-redis-test-data)" ]; then
        docker volume rm evolveme-redis-test-data 2>/dev/null || true
    fi

    if [ "$(docker volume ls -q -f name=evolveme-minio-test-data)" ]; then
        docker volume rm evolveme-minio-test-data 2>/dev/null || true
    fi
}

# Function to run database migrations
run_migrations() {
    echo -e "${YELLOW}Running database migrations...${NC}"
    DATABASE_URL="postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME" sqlx migrate run
}

# Function to prepare SQLx queries
prepare_sqlx() {
    echo -e "${YELLOW}Preparing SQLx query metadata...${NC}"
    DATABASE_URL="postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME" cargo sqlx prepare
}

# Function to run tests
run_tests() {
    echo -e "${YELLOW}Running tests...${NC}"
    if [ -n "${TEST_FILE:-}" ]; then
        echo -e "${YELLOW}Running all tests in file: $TEST_FILE${NC}"
        if [ "${SHOW_OUTPUT:-false}" = "true" ]; then
            TEST_LOG=1 RUST_BACKTRACE=1 cargo test --test "$TEST_FILE" -- --nocapture
        else
            RUST_BACKTRACE=1 cargo test --test "$TEST_FILE"
        fi
    elif [ -n "${TEST_NAME:-}" ]; then
        echo -e "${YELLOW}Running test matching pattern: $TEST_NAME${NC}"
        if [ "${SHOW_OUTPUT:-false}" = "true" ]; then
            TEST_LOG=1 RUST_BACKTRACE=1 cargo test "$TEST_NAME" -- --nocapture
        else
            RUST_BACKTRACE=1 cargo test "$TEST_NAME"
        fi
    else
        if [ "${SHOW_OUTPUT:-false}" = "true" ]; then
            TEST_LOG=1 RUST_BACKTRACE=1 cargo test -- --nocapture
        else
            RUST_BACKTRACE=1 cargo test
        fi
    fi
}

# Main script execution
main() {
    # Check for required commands
    for cmd in pg_isready sqlx cargo; do
        if ! command -v "$cmd" &> /dev/null; then
            echo -e "${RED}Error: $cmd is not installed.${NC}"
            exit 1
        fi
    done

    # Prompt for database password if not set
    if [ -z "$DB_PASSWORD" ]; then
        read -sp "Enter PostgreSQL password: " DB_PASSWORD
        echo
    fi

    # Execute steps
    check_postgres
    check_redis
    check_minio
    run_migrations
    prepare_sqlx
    run_tests
    clean_up

    echo -e "${GREEN}Test run and cleanup completed successfully!${NC}"
}

# Help function
show_help() {
    echo "Usage: $0 [options]"
    echo "Options:"
    echo "  -h, --help           Show this help message"
    echo "  -u, --user           PostgreSQL username (default: postgres)"
    echo "  -p, --port           PostgreSQL port (default: 5432)"
    echo "  -d, --database       Database name (default: evolveme_db)"
    echo "  --host               PostgreSQL host (default: localhost)"
    echo "  -t, --test           Run a specific test by name pattern"
    echo "  -f, --file           Run all tests in a specific file (e.g., admin_integration_test)"
    echo "  -v, --verbose        Show test output (print statements, etc.)"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    key="$1"
    case $key in
        -h|--help)
            show_help
            exit 0
            ;;
        -u|--user)
            DB_USER="$2"
            shift 2
            ;;
        -p|--port)
            DB_PORT="$2"
            shift 2
            ;;
        -d|--database)
            DB_NAME="$2"
            shift 2
            ;;
        --host)
            DB_HOST="$2"
            shift 2
            ;;
        -t|--test)
            TEST_NAME="$2"
            shift 2
            ;;
        -f|--file)
            TEST_FILE="$2"
            shift 2
            ;;
        -v|--verbose)
            SHOW_OUTPUT="true"
            shift
            ;;
        *)
            echo "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Run the main function
main