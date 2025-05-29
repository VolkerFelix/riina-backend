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

export APP_ENVIRONMENT=${APP_ENVIRONMENT:-test}
# PostgreSQL Configuration
export POSTGRES__DATABASE__USER=${POSTGRES__DATABASE__USER:-postgres}
export POSTGRES__DATABASE__PASSWORD=${POSTGRES__DATABASE__PASSWORD:-postgres}
# Actix Web Configuration
export APP__APPLICATION__USER=${APP__APPLICATION__USER:-testuser}
export APP__APPLICATION__PASSWORD=${APP__APPLICATION__PASSWORD:-testpassword}
# Redis Config
export REDIS__REDIS__PASSWORD=${REDIS__REDIS__PASSWORD:-redis}
# LLM Config
export LLM__LLM__SERVICE_URL=${LLM__LLM__SERVICE_URL:-http://localhost:8082}
export LLM__LLM__MODEL=${LLM__LLM__MODEL:-llama3.2:3b-instruct-q4_K_M}


# Default configuration
DB_HOST="localhost"
DB_PORT=5432
DB_USER=${POSTGRES__DATABASE__USER}
DB_NAME="evolveme_db"
DB_PASSWORD=${POSTGRES__DATABASE__PASSWORD}

# Explicitly set DATABASE_URL for SQLx
export DATABASE_URL="postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME"

# Function to check if PostgreSQL is running
check_postgres() {
    if ! pg_isready -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" > /dev/null 2>&1; then
        echo -e "${RED}Error: PostgreSQL is not running or not accessible.${NC}"
        echo -e "${YELLOW}Please ensure PostgreSQL is running on $DB_HOST:$DB_PORT${NC}"
        exit 1
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
    if [ -n "${TEST_NAME:-}" ]; then
        echo -e "${YELLOW}Running test matching pattern: $TEST_NAME${NC}"
        if [ "${SHOW_OUTPUT:-false}" = "true" ]; then
            RUST_BACKTRACE=1 cargo test "$TEST_NAME" -- --nocapture
        else
            RUST_BACKTRACE=1 cargo test "$TEST_NAME"
        fi
    else
        if [ "${SHOW_OUTPUT:-false}" = "true" ]; then
            RUST_BACKTRACE=1 cargo test -- --nocapture
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
    run_migrations
    prepare_sqlx
    run_tests

    echo -e "${GREEN}Test run completed successfully!${NC}"
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