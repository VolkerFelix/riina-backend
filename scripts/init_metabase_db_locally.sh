#!/bin/bash

# Script to initialize Metabase database in PostgreSQL

set -e

# Load environment variables
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
fi

DB_USER=${POSTGRES__DATABASE__USER:-postgres}
DB_PASSWORD=${POSTGRES__DATABASE__PASSWORD:-postgres}
DB_HOST=${1:-localhost}
DB_PORT=${2:-5432}

echo "Creating Metabase database..."

# Create metabase database if it doesn't exist
PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -tc "SELECT 1 FROM pg_database WHERE datname = 'metabase'" | grep -q 1 || \
PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -c "CREATE DATABASE metabase;"

echo "Metabase database created successfully!"
