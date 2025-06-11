#!/bin/bash
# scripts/run-migrations.sh - Simplified and efficient version

set -e

echo "🔄 Starting database migrations..."

# Wait for PostgreSQL (simplified)
until pg_isready -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" >/dev/null 2>&1; do
  sleep 1
done
echo "✅ PostgreSQL ready"

# Create migrations table
psql -v ON_ERROR_STOP=1 >/dev/null 2>&1 <<-EOSQL
CREATE TABLE IF NOT EXISTS _migrations (
    id SERIAL PRIMARY KEY,
    filename VARCHAR(255) UNIQUE NOT NULL,
    executed_at TIMESTAMPTZ DEFAULT NOW()
);
EOSQL

echo "📋 Migration tracking ready"

# Process migrations
migration_count=0
for migration_file in $(ls /migrations/*.sql 2>/dev/null | sort); do
    filename=$(basename "$migration_file")
    
    # Skip if already executed
    if psql -tAc "SELECT 1 FROM _migrations WHERE filename = '$filename'" 2>/dev/null | grep -q 1; then
        continue
    fi
    
    echo "▶️  $filename"
    
    # Run migration
    if psql -v ON_ERROR_STOP=1 -f "$migration_file" >/dev/null 2>&1; then
        psql -c "INSERT INTO _migrations (filename) VALUES ('$filename') ON CONFLICT DO NOTHING" >/dev/null 2>&1
        echo "✅ $filename"
        migration_count=$((migration_count + 1))
    else
        echo "❌ $filename failed"
    fi
done

echo "🎉 Completed: $migration_count new migrations"
exit 0