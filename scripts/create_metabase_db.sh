#!/bin/bash

# Create metabase database for Metabase application storage
# This is separate from the main evolveme_db that Metabase will analyze

echo "Creating Metabase database..."

# Check if database already exists
DB_EXISTS=$(psql -h postgres -U "$PGUSER" -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname='metabase'")

if [ "$DB_EXISTS" = "1" ]; then
    echo "Metabase database already exists, skipping creation"
else
    echo "Creating metabase database..."
    psql -h postgres -U "$PGUSER" -d postgres -c "CREATE DATABASE metabase;"
    echo "Metabase database created successfully"
fi

echo "Metabase database setup complete"