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

# Allow to skip Docker if dockerized services are already running
if [[ -z "${SKIP_DOCKER}" ]]
then
    # Start PostgreSQL container
    docker run \
    -e POSTGRES_USER=${POSTGRES__DATABASE__USER} \
    -e POSTGRES_PASSWORD=${POSTGRES__DATABASE__PASSWORD} \
    -e POSTGRES_DB=evolveme_db \
    -p 5432:5432 \
    -d postgres \
    postgres -N 1000

    # Start Redis container
    docker run \
    -e REDIS_PASSWORD=${REDIS_PASSWORD} \
    -p 6379:6379 \
    -d redis \
    redis-server --requirepass ${REDIS_PASSWORD}
fi

# Keep pinging Postgres until it's ready to accept commands
export PGPASSWORD="${POSTGRES__DATABASE__PASSWORD}"
until psql -h "localhost" -U "${POSTGRES__DATABASE__USER}" -p 5432 -d evolveme_db -c '\q'; do
    >&2 echo "Postgres is still unavailable - sleeping"
    sleep 1
done

# Keep pinging Redis until it's ready to accept commands
until redis-cli -h localhost -p 6379 -a ${REDIS_PASSWORD} ping; do
    >&2 echo "Redis is still unavailable - sleeping"
    sleep 1
done

>&2 echo "Postgres and Redis are up and running - running migrations now!"

DATABASE_URL=postgres://${POSTGRES__DATABASE__USER}:${POSTGRES__DATABASE__PASSWORD}@localhost:5432/evolveme_db
export DATABASE_URL
sqlx database create
sqlx migrate run
cargo sqlx prepare --database-url $DATABASE_URL

>&2 echo "Postgres has been migrated, ready to go!"