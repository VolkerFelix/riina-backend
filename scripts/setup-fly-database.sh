#!/bin/bash

# Script to create the evolveme_db database and run migrations on Fly.io PostgreSQL
# Usage: ./setup-fly-database.sh

set -e  # Exit on error

echo "🚀 Setting up database on Fly.io PostgreSQL..."

echo ""
echo "📦 Creating evolveme_db database if it doesn't exist..."

# Create the database
echo "CREATE DATABASE evolveme_db;" | fly postgres connect -a riina-db -c "psql" || echo "Database might already exist, continuing..."

echo ""
echo "✅ Database evolveme_db is ready"

# Check if migrations directory exists
if [ ! -d "migrations" ]; then
    echo "❌ Error: migrations directory not found!"
    echo "Expected path: $(pwd)/migrations"
    exit 1
fi

# Get list of migration files
MIGRATIONS=(migrations/*.sql)

echo ""
echo "📋 Found ${#MIGRATIONS[@]} migration files:"
for migration in "${MIGRATIONS[@]}"; do
    echo "  - $(basename "$migration")"
done

echo ""
echo "⚡ Running migrations on evolveme_db database..."

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
echo "📦 Running all migrations on evolveme_db database..."

# Run all migrations on the specific database
fly postgres connect -a riina-db -d evolveme_db -c "psql -v ON_ERROR_STOP=1 -f -" < "$TEMP_FILE"

# Clean up
rm -f "$TEMP_FILE"

echo ""
echo "🎉 Database setup completed successfully!"
echo ""
echo "💡 You can verify the database schema by running:"
echo "   fly postgres connect -a riina-db -d evolveme_db"
echo "   Then in psql use: \\dt to list all tables"