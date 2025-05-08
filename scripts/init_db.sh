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

# Allow to skip Docker if a dockerized Postgres database is already running
if [[ -z "${SKIP_DOCKER}" ]]
then
    docker run \
    -e POSTGRES_USER=${POSTGRES__DATABASE__USER} \
    -e POSTGRES_PASSWORD=${POSTGRES__DATABASE__PASSWORD} \
    -e POSTGRES_DB=areum_db \
    -p 5432:5432 \
    -d postgres \
    postgres -N 1000
fi

# Keep pinging Postgres until it's ready to accept commands
export PGPASSWORD="${POSTGRES__DATABASE__PASSWORD}"
until psql -h "localhost" -U "${POSTGRES__DATABASE__USER}" -p 5432 -d areum_db -c '\q'; do
    >&2 echo "Postgres is still unavailable - sleeping"
    sleep 1
done

>&2 echo "Postgres is up and running on port 5432 - running migrations now!"

DATABASE_URL=postgres://${POSTGRES__DATABASE__USER}:${POSTGRES__DATABASE__PASSWORD}@localhost:5432/areum_db
export DATABASE_URL
sqlx database create
sqlx migrate run
cargo sqlx prepare --database-url $DATABASE_URL

>&2 echo "Postgres has been migrated, ready to go!"