#!/bin/bash

# Setup Redis as a regular Fly.io app (not Upstash) for development
set -e

echo "🔴 Setting up Redis as a regular Fly.io app for development..."

# Check if fly CLI is installed
if ! command -v fly &> /dev/null; then
    echo "❌ Fly CLI not found. Please install it first"
    exit 1
fi

# Create a temporary directory for Redis app
REDIS_DIR=$(mktemp -d)
cd "$REDIS_DIR"

echo "📁 Creating Redis app configuration in $REDIS_DIR..."

# Create fly.toml for Redis
cat > fly.toml << 'EOF'
app = "evolveme-redis-dev"
primary_region = "ams"

[build]
  image = "redis:7-alpine"

[processes]
  app = "redis-server --requirepass $REDIS_PASSWORD --appendonly yes --dir /data"

[[services]]
  processes = ["app"]
  protocol = "tcp"
  internal_port = 6379
  
  [[services.ports]]
    port = 6379

[env]
  REDIS_PASSWORD = "dev-redis-password-change-me"

[[mounts]]
  source = "redis_data"
  destination = "/data"
EOF

echo "🚀 Creating and deploying Redis app..."

# Create the app
fly apps create evolveme-redis-dev || echo "App already exists"

# Create volume for persistence
echo "💾 Creating volume for Redis data..."
fly volumes create redis_data --region ams --size 1 --app evolveme-redis-dev || echo "Volume might already exist"

# Set Redis password as secret
echo "🔐 Setting Redis password..."
REDIS_PASSWORD="dev-redis-$(openssl rand -hex 16)"
fly secrets set REDIS_PASSWORD="$REDIS_PASSWORD" --app evolveme-redis-dev

# Deploy Redis
echo "🏗️ Deploying Redis..."
fly deploy --app evolveme-redis-dev

# Wait for deployment
echo "⏳ Waiting for Redis to start..."
sleep 10

echo ""
echo "✅ Redis app created and deployed successfully!"
echo ""
echo "Redis connection details:"
echo "- App: evolveme-redis-dev"
echo "- Internal URL: evolveme-redis-dev.internal:6379"
echo "- Password: $REDIS_PASSWORD"
echo ""
echo "To connect your backend app to this Redis:"
echo "fly secrets set REDIS__REDIS__HOST=evolveme-redis-dev.internal --app riina-backend-dev"
echo "fly secrets set REDIS__REDIS__PORT=6379 --app riina-backend-dev"
echo "fly secrets set REDIS__REDIS__PASSWORD=$REDIS_PASSWORD --app riina-backend-dev"
echo ""
echo "To check Redis status:"
echo "fly status --app evolveme-redis-dev"
echo "fly logs --app evolveme-redis-dev"

# Clean up
cd - > /dev/null
rm -rf "$REDIS_DIR"