#!/bin/bash

# Deploy EvolveMe Backend Development Instance to Fly.io
set -e

echo "🚀 Deploying EvolveMe Backend Development Instance to Fly.io..."

# Check if fly CLI is installed
if ! command -v fly &> /dev/null; then
    echo "❌ Fly CLI not found. Please install it first:"
    echo "   curl -L https://fly.io/install.sh | sh"
    exit 1
fi

# Check if logged into Fly
if ! fly auth whoami &> /dev/null; then
    echo "❌ Not logged into Fly.io. Please run 'fly auth login' first"
    exit 1
fi

# Create the app if it doesn't exist
echo "📱 Creating development app if it doesn't exist..."
fly apps create evolveme-backend-dev || echo "App already exists"

# Deploy the application
echo "🏗️ Deploying application..."
fly deploy --config fly-dev.toml

echo "✅ Development backend deployed successfully!"
echo "🌐 Access it at: https://evolveme-backend-dev.fly.dev"
echo ""
echo "Next steps:"
echo "1. Set up PostgreSQL database: ./scripts/setup-dev-database.sh"
echo "2. Set up Redis: ./scripts/setup-dev-redis.sh"
echo "3. Run migrations: ./scripts/run-dev-migrations.sh"