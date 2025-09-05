#!/bin/bash

# Setup PostgreSQL database for development instance on Fly.io
set -e

echo "🗄️ Setting up PostgreSQL database for development..."

# Check if fly CLI is installed
if ! command -v fly &> /dev/null; then
    echo "❌ Fly CLI not found. Please install it first"
    exit 1
fi

# Create PostgreSQL app for development
echo "📊 Creating PostgreSQL database..."
fly postgres create riina-db-dev --region ams --vm-size shared-cpu-1x --volume-size 1

# Attach database to the backend app
echo "🔗 Attaching database to backend app..."
fly postgres attach riina-db-dev --app riina-backend-dev

echo "✅ PostgreSQL database created and attached!"
echo ""
echo "Database details:"
echo "- Name: riina-db-dev"
echo "- Region: ams"
echo "- VM Size: shared-cpu-1x"
echo "- Volume: 1GB"
echo ""
echo "Connection details will be automatically set as environment variables:"
echo "- DATABASE_URL"
echo "- POSTGRES_HOST, POSTGRES_PORT, POSTGRES_DB, etc."