use chrono::Utc;
use redis::AsyncCommands;
use uuid::Uuid;
use std::sync::Arc;
use redis::Client as RedisClient;

use crate::models::game_events::GameEvent;

/// Publish a chat message event to Redis for WebSocket broadcasting to team members
pub async fn publish_chat_message(
    redis_client: &Arc<RedisClient>,
    team_id: Uuid,
    message_id: Uuid,
    user_id: Uuid,
    username: String,
    message: String,
) -> Result<(), String> {
    let event = GameEvent::TeamChatMessage {
        message_id,
        team_id,
        user_id,
        username,
        message,
        timestamp: Utc::now(),
    };

    publish_team_event(redis_client, team_id, event).await
}

/// Publish a chat message edited event
pub async fn publish_chat_message_edited(
    redis_client: &Arc<RedisClient>,
    team_id: Uuid,
    message_id: Uuid,
    user_id: Uuid,
    username: String,
    message: String,
) -> Result<(), String> {
    let event = GameEvent::TeamChatMessageEdited {
        message_id,
        team_id,
        user_id,
        username,
        message,
        edited_at: Utc::now(),
    };

    publish_team_event(redis_client, team_id, event).await
}

/// Publish a chat message deleted event
pub async fn publish_chat_message_deleted(
    redis_client: &Arc<RedisClient>,
    team_id: Uuid,
    message_id: Uuid,
    user_id: Uuid,
) -> Result<(), String> {
    let event = GameEvent::TeamChatMessageDeleted {
        message_id,
        team_id,
        user_id,
        timestamp: Utc::now(),
    };

    publish_team_event(redis_client, team_id, event).await
}

/// Generic function to publish a team event to Redis
async fn publish_team_event(
    redis_client: &Arc<RedisClient>,
    team_id: Uuid,
    event: GameEvent,
) -> Result<(), String> {
    let mut conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("Failed to get Redis connection: {}", e))?;

    // Publish to team-specific channel so all team members receive it
    let channel = format!("game:events:team:{}", team_id);
    let redis_message = serde_json::to_string(&event)
        .map_err(|e| format!("Failed to serialize team event: {}", e))?;

    conn.publish::<_, _, ()>(&channel, redis_message)
        .await
        .map_err(|e| format!("Failed to publish team event to Redis: {}", e))?;

    tracing::info!(
        "Published team event to channel {} for team {}",
        channel,
        team_id
    );

    Ok(())
}
