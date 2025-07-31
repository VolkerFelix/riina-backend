#!/bin/bash

# Script to run database migrations on Fly.io PostgreSQL
# Usage: ./run-fly-migrations-simple.sh

set -e  # Exit on error

echo "🚀 Running database migrations on Fly.io PostgreSQL..."

# Check if migrations directory exists
if [ ! -d "migrations" ]; then
    echo "❌ Error: migrations directory not found!"
    echo "Expected path: $(pwd)/migrations"
    exit 1
fi

# Get list of migration files
MIGRATIONS=(migrations/*.sql)

echo "📋 Found ${#MIGRATIONS[@]} migration files:"
for migration in "${MIGRATIONS[@]}"; do
    echo "  - $(basename "$migration")"
done

echo ""
echo "⚡ Running migrations..."

# Combine all migrations into one file
TEMP_FILE=$(mktemp)
echo "-- Combined migrations file" > "$TEMP_FILE"
echo "" >> "$TEMP_FILE"

for migration in "${MIGRATIONS[@]}"; do
    echo "" >> "$TEMP_FILE"
    echo "-- Migration: $(basename "$migration")" >> "$TEMP_FILE"
    echo "-- ========================================" >> "$TEMP_FILE"
    cat "$migration" >> "$TEMP_FILE"
    echo "" >> "$TEMP_FILE"
done

echo ""
echo "📦 Running all migrations in a single transaction..."

# Run all migrations at once
fly postgres connect -a evolveme-db-dev -d evolveme_db -c "psql -v ON_ERROR_STOP=1 -f -" < "$TEMP_FILE"

# Clean up
rm -f "$TEMP_FILE"

echo ""
echo "🎉 Migrations completed!"
echo ""
echo "💡 You can verify the database schema by running:"
echo "   fly postgres connect -a evolveme-db"
echo "   Then in psql use: \\dt to list all tables"