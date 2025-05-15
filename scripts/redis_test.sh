#!/bin/bash
# Redis test script to diagnose pub/sub issues

export REDIS__REDIS__PASSWORD=superSecureRedisPassword123


# Check if REDIS_PASSWORD is set
if [ -z "$REDIS__REDIS__PASSWORD" ]; then
  echo "REDIS_PASSWORD environment variable is required"
  exit 1
fi

echo "========== REDIS PUBSUB TEST SCRIPT =========="
echo 

# Step 1: Test basic connectivity
echo "Step 1: Testing basic Redis connectivity"
redis-cli -a "$REDIS__REDIS__PASSWORD" ping
if [ $? -ne 0 ]; then
  echo "❌ Failed to connect to Redis"
  exit 1
fi
echo "✅ Redis connection successful"
echo

# Step 2: List all patterns 
echo "Step 2: Listing all existing Redis pubsub channels"
CHANNELS=$(redis-cli -a "$REDIS__REDIS__PASSWORD" pubsub channels "*")
echo "Existing channels: $CHANNELS"
echo

# Step 3: Monitor Redis for a few seconds to see all pub/sub activity
echo "Step 3: Monitoring Redis (will run for 5 seconds)"
echo "Any pub/sub activity will be shown below:"
echo "-----------------------------------------"
timeout 5 redis-cli -a "$REDIS__REDIS__PASSWORD" monitor | grep -i "SUBSCRIBE\|PUBLISH"
echo "-----------------------------------------"
echo

# Step 4: Monitor a specific channel
echo "Step 4: Check for explicit Redis subscriptions"
echo "Enter the user ID to check (from the JWT token):"
read USER_ID
CHANNEL="evolveme:events:user:$USER_ID"
echo "Checking for subscribers on channel: $CHANNEL"
SUBS=$(redis-cli -a "$REDIS__REDIS__PASSWORD" pubsub numsub "$CHANNEL")
echo "Subscribers: $SUBS"
echo

# Step 5: Try to publish a test message
echo "Step 5: Publish test message to channel"
TEST_MSG="{\"event_type\":\"test_message\",\"user_id\":\"$USER_ID\",\"message\":\"Test from CLI\",\"timestamp\":\"$(date -u +"%Y-%m-%dT%H:%M:%SZ")\"}"
echo "Message: $TEST_MSG"
RECEIVERS=$(redis-cli -a "$REDIS__REDIS__PASSWORD" publish "$CHANNEL" "$TEST_MSG")
echo "Message published to $RECEIVERS receivers"
echo

# Step 6: Check the Redis implementation
echo "Step 6: Verification checklist"
echo "□ Ensure Redis is running on the default port (6379)"
echo "□ Verify Redis password is the same in the app and test environment"
echo "□ Check that Redis is not in protected mode or blocked by firewall"
echo "□ Verify that Redis pubsub channels are named consistently"
echo "□ Check logs for WebSocket actor startup and Redis connection"
echo

echo "========== END OF TEST =========="