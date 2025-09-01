#!/bin/bash

# Setup environment secrets for development instance
set -e

echo "🔐 Setting up environment secrets for development..."

# Check if fly CLI is installed
if ! command -v fly &> /dev/null; then
    echo "❌ Fly CLI not found. Please install it first"
    exit 1
fi

# Set secrets for development
echo "🔑 Setting secrets..."
fly secrets set \
  JWT_SECRET="dev-jwt-secret-$(openssl rand -hex 32)" \
  APP_ENVIRONMENT="production" \
  APP__APPLICATION__USER="evolveme_dev_user" \
  APP__APPLICATION__PASSWORD="$(openssl rand -base64 32)" \
  POSTGRES__DATABASE__DB_NAME="evolveme_db" \
  POSTGRES__DATABASE__HOST="evolveme-db-dev.internal" \
  POSTGRES__DATABASE__USER="postgres" \
  POSTGRES__DATABASE__PASSWORD="$(openssl rand -base64 32)" \
  POSTGRES__DATABASE__PORT="5432" \
  REDIS__REDIS__HOST="evolveme-redis-dev.internal" \
  REDIS__REDIS__PORT="6379" \
  REDIS__REDIS__PASSWORD="$(openssl rand -base64 32)" \
  --app riina-backend-dev

echo ""
echo "✅ Environment secrets set successfully!"
echo ""
echo "Secrets that were set:"
echo "- JWT_SECRET (auto-generated)"
echo "- APP__APPLICATION__USER"
echo "- APP__APPLICATION__PASSWORD (auto-generated)"
echo "- POSTGRES__DATABASE__USER"
echo "- POSTGRES__DATABASE__PASSWORD (auto-generated)"
echo "- REDIS__REDIS__PASSWORD (auto-generated)"
echo "- APP_ENVIRONMENT"
echo "DB and Redis passwords need to be set manually after setup"
echo ""