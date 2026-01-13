use chrono::Utc;
use redis::AsyncCommands;
use uuid::Uuid;
use std::sync::Arc;
use redis::Client as RedisClient;

use crate::models::game_events::GameEvent;

/// Publish a chat message event to Redis for WebSocket broadcasting to team members
#[allow(clippy::too_many_arguments)]
pub async fn publish_chat_message(
    redis_client: &Arc<RedisClient>,
    team_id: Uuid,
    message_id: Uuid,
    user_id: Uuid,
    username: String,
    profile_picture_url: Option<String>,
    message: String,
    gif_url: Option<String>,
) -> Result<(), String> {
    let event = GameEvent::TeamChatMessage {
        message_id,
        team_id,
        user_id,
        username,
        profile_picture_url,
        message,
        gif_url,
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

/// Send chat message received notification to a specific user
pub async fn send_chat_message_received_to_user(
    redis_client: &Arc<RedisClient>,
    recipient_id: Uuid,
    team_id: Uuid,
    message_id: Uuid,
    sender_username: String,
    team_name: String,
) -> Result<(), String> {
    let event = GameEvent::ChatMessageReceived {
        recipient_id,
        team_id,
        message_id,
        sender_username: sender_username.clone(),
        team_name: team_name.clone(),
        timestamp: Utc::now(),
    };

    let mut conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("Failed to get Redis connection: {e}"))?;

    // Send to user-specific channel only
    let user_channel = format!("game:events:user:{recipient_id}");
    let event_message = serde_json::to_string(&event)
        .map_err(|e| format!("Failed to serialize chat message received event: {e}"))?;

    conn.publish::<_, _, ()>(&user_channel, event_message)
        .await
        .map_err(|e| format!("Failed to publish chat message received event to Redis: {e}"))?;

    tracing::info!(
        "📬 Sent chat_message_received to user {} from {} in team {}",
        recipient_id,
        sender_username,
        team_name
    );

    Ok(())
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
        .map_err(|e| format!("Failed to get Redis connection: {e}"))?;

    // Publish to team-specific channel so all team members receive it
    let channel = format!("game:events:team:{team_id}");
    let redis_message = serde_json::to_string(&event)
        .map_err(|e| format!("Failed to serialize team event: {e}"))?;

    conn.publish::<_, _, ()>(&channel, redis_message)
        .await
        .map_err(|e| format!("Failed to publish team event to Redis: {e}"))?;

    tracing::info!(
        "Published team event to channel {} for team {}",
        channel,
        team_id
    );

    Ok(())
}
