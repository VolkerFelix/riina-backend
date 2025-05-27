#!/usr/bin/env bash
set -eo pipefail

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

# Load .env file if not in CI environment (e.g., GitHub Actions)
if [ -z "${CI}" ]; then
    # Load .env file
    PARENT_DIR="$(dirname "$(pwd)")"
    if [ -f .env ]; then
        export $(grep -v '^#' .env | xargs)
    else
        echo ".env file not found."
        exit 1
    fi
else
    echo "CI environment detected - Skipping loading .env file."
fi

# Default LLM model - can be overridden via environment variable
LLM_MODEL=${LLM_MODEL:-"llama3.2:3b-instruct-q4_K_M"}
OLLAMA_CONTAINER_NAME="evolveme-ollama"

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

# Allow to skip Docker if dockerized services are already running
if [[ -z "${SKIP_DOCKER}" ]]
then
    echo "🐳 Starting Docker containers..."

    # Start PostgreSQL container
    echo "Starting PostgreSQL container..."
    if container_running "evolveme-postgres"; then
        echo "PostgreSQL container already running"
    else
        if container_exists "evolveme-postgres"; then
            echo "Starting existing PostgreSQL container..."
            docker start evolveme-postgres
        else
            echo "Creating new PostgreSQL container..."
            docker run \
                --name evolveme-postgres \
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
    if container_running "evolveme-redis"; then
        echo "Redis container already running"
    else
        if container_exists "evolveme-redis"; then
            echo "Starting existing Redis container..."
            docker start evolveme-redis
        else
            echo "Creating new Redis container..."
            docker run \
                --name evolveme-redis \
                -e REDIS_PASSWORD=${REDIS__REDIS__PASSWORD} \
                -p 6379:6379 \
                -d redis \
                redis-server --requirepass ${REDIS__REDIS__PASSWORD}
        fi
    fi

    # Start Ollama container
    echo "Starting Ollama container..."
    if container_running "$OLLAMA_CONTAINER_NAME"; then
        echo "Ollama container already running"
    else
        if container_exists "$OLLAMA_CONTAINER_NAME"; then
            echo "Starting existing Ollama container..."
            docker start "$OLLAMA_CONTAINER_NAME"
        else
            echo "Creating new Ollama container..."
            # Create a named volume for Ollama models to persist between container restarts
            docker volume create ollama-models 2>/dev/null || true
            
            docker run \
                --name "$OLLAMA_CONTAINER_NAME" \
                -p 11434:11434 \
                -v ollama-models:/root/.ollama \
                -d ollama/ollama
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

# Wait for Ollama
wait_for_service "Ollama" "curl -s http://localhost:11434/api/tags"

echo "✅ All services are ready!"

# Pull the LLM model if it doesn't exist
echo "🤖 Setting up LLM model..."
if docker exec "$OLLAMA_CONTAINER_NAME" ollama list | grep -q "$LLM_MODEL"; then
    echo "Model $LLM_MODEL already exists"
else
    echo "Pulling model $LLM_MODEL (this may take a few minutes)..."
    docker exec "$OLLAMA_CONTAINER_NAME" ollama pull "$LLM_MODEL"
    echo "✅ Model $LLM_MODEL pulled successfully"
fi

# Test the LLM model
echo "🧪 Testing LLM model..."
test_response=$(curl -s -X POST http://localhost:11434/api/generate \
    -H "Content-Type: application/json" \
    -d "{
        \"model\": \"$LLM_MODEL\",
        \"prompt\": \"Hello! Respond with just 'OK' if you can understand this.\",
        \"stream\": false,
        \"options\": {\"num_predict\": 5}
    }" | grep -o '"response":"[^"]*"' | cut -d'"' -f4 2>/dev/null || echo "")

if [[ "$test_response" =~ [Oo][Kk] ]]; then
    echo "✅ LLM model is working correctly"
else
    echo "⚠️  LLM model test returned: '$test_response'"
    echo "The model is running but may need warming up"
fi

# Database setup
echo "🗄️ Setting up database..."
DATABASE_URL=postgres://${POSTGRES__DATABASE__USER}:${POSTGRES__DATABASE__PASSWORD}@localhost:5432/evolveme_db
export DATABASE_URL
sqlx database create
sqlx migrate run
cargo sqlx prepare --database-url $DATABASE_URL

echo "✅ Database has been migrated and is ready!"

# Print service status
echo ""
echo "🎉 All services are up and running!"
echo "========================================"
echo "PostgreSQL: localhost:5432"
echo "Redis: localhost:6379"
echo "Ollama: http://localhost:11434"
echo "LLM Model: $LLM_MODEL"
echo ""

# Optional: Update configuration file
if [ -f "configuration/local.yml" ]; then
    echo "📝 Configuration suggestions for configuration/local.yml:"
    echo "llm:"
    echo "  service_url: \"http://localhost:11434\""
    echo "  model_name: \"$LLM_MODEL\""
    echo "  timeout_seconds: 30"
    echo "  max_retries: 3"
    echo ""
fi