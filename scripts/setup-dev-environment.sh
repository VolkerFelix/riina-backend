#!/bin/bash

# Complete setup script for EvolveMe Backend Development Environment on Fly.io
set -e

echo "🚀 Setting up complete EvolveMe Backend Development Environment on Fly.io"
echo "=================================================================="

# Step 1: Deploy the application
echo ""
echo "Step 1: Deploying application..."
./scripts/deploy-dev.sh

# Step 2: Setup database
echo ""
echo "Step 2: Setting up PostgreSQL database..."
read -p "Press Enter to continue or Ctrl+C to skip..."
./scripts/setup-dev-database.sh

# Step 3: Setup Redis
echo ""
echo "Step 3: Setting up Redis..."
read -p "Press Enter to continue or Ctrl+C to skip..."
./scripts/setup-dev-redis.sh

# Step 4: Setup secrets
echo ""
echo "Step 4: Setting up environment secrets..."
read -p "Press Enter to continue or Ctrl+C to skip..."
./scripts/setup-dev-secrets.sh

# Step 5: Get Redis password and set it
echo ""
echo "Step 5: Setting Redis password..."
echo "Getting Redis password..."
REDIS_PASSWORD=$(fly redis status evolveme-redis-dev | grep "Password:" | awk '{print $2}' || echo "")

if [ -n "$REDIS_PASSWORD" ]; then
    fly secrets set REDIS__REDIS__PASSWORD="$REDIS_PASSWORD" --app evolveme-backend-dev
    echo "✅ Redis password set automatically"
else
    echo "⚠️  Could not automatically get Redis password. Please set it manually:"
    echo "   fly redis status evolveme-redis-dev"
    echo "   fly secrets set REDIS__REDIS__PASSWORD=<password> --app evolveme-backend-dev"
fi

# Step 6: Run migrations
echo ""
echo "Step 6: Running database migrations..."
read -p "Press Enter to continue or Ctrl+C to skip..."
./scripts/run-dev-migrations.sh

# Step 7: Final deployment with all configs
echo ""
echo "Step 7: Final deployment with all configurations..."
fly deploy --config fly-dev.toml

echo ""
echo "🎉 Development environment setup complete!"
echo "=================================================================="
echo ""
echo "Your development backend is available at:"
echo "🌐 https://evolveme-backend-dev.fly.dev"
echo ""
echo "Resources created:"
echo "- App: evolveme-backend-dev"
echo "- Database: evolveme-db-dev"
echo "- Redis: evolveme-redis-dev"
echo ""
echo "To check status:"
echo "- fly status --app evolveme-backend-dev"
echo "- fly logs --app evolveme-backend-dev"
echo ""
echo "To update frontend to use this backend:"
echo "Set REACT_APP_API_URL=https://evolveme-backend-dev.fly.dev/api"