use chrono::Utc;
use redis::AsyncCommands;
use uuid::Uuid;
use std::sync::Arc;
use redis::Client as RedisClient;

use crate::models::player_pool::{PlayerPoolEvent, PlayerPoolEventType};

/// Publish a player pool event to Redis for WebSocket broadcasting
pub async fn publish_player_pool_event(
    redis_client: &Arc<RedisClient>,
    event: PlayerPoolEvent,
) -> Result<(), String> {
    let mut conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("Failed to get Redis connection: {}", e))?;

    let channel = "player_pool_events";
    let message = serde_json::to_string(&event)
        .map_err(|e| format!("Failed to serialize player pool event: {}", e))?;

    conn.publish::<_, _, ()>(channel, message)
        .await
        .map_err(|e| format!("Failed to publish player pool event: {}", e))?;

    tracing::info!(
        "Published player pool event: {} for user {}",
        event.event_type.as_str(),
        event.user_id
    );

    Ok(())
}

/// Publish a "player joined pool" event
pub async fn publish_player_joined(
    redis_client: &Arc<RedisClient>,
    user_id: Uuid,
    username: String,
    league_id: Option<Uuid>,
) -> Result<(), String> {
    let event = PlayerPoolEvent {
        event_type: PlayerPoolEventType::PlayerJoined,
        user_id,
        username,
        league_id,
        team_id: None,
        team_name: None,
        timestamp: Utc::now(),
    };

    publish_player_pool_event(redis_client, event).await
}

/// Publish a "player left pool" event (user went inactive)
pub async fn publish_player_left(
    redis_client: &Arc<RedisClient>,
    user_id: Uuid,
    username: String,
    league_id: Option<Uuid>,
) -> Result<(), String> {
    let event = PlayerPoolEvent {
        event_type: PlayerPoolEventType::PlayerLeft,
        user_id,
        username,
        league_id,
        team_id: None,
        team_name: None,
        timestamp: Utc::now(),
    };

    publish_player_pool_event(redis_client, event).await
}

/// Publish a "player assigned to team" event
pub async fn publish_player_assigned(
    redis_client: &Arc<RedisClient>,
    user_id: Uuid,
    username: String,
    league_id: Option<Uuid>,
    team_id: Uuid,
    team_name: String,
) -> Result<(), String> {
    let event = PlayerPoolEvent {
        event_type: PlayerPoolEventType::PlayerAssigned,
        user_id,
        username,
        league_id,
        team_id: Some(team_id),
        team_name: Some(team_name),
        timestamp: Utc::now(),
    };

    publish_player_pool_event(redis_client, event).await
}

/// Publish a "player left team" event
pub async fn publish_player_left_team(
    redis_client: &Arc<RedisClient>,
    user_id: Uuid,
    username: String,
    league_id: Option<Uuid>,
    team_id: Uuid,
    team_name: String,
) -> Result<(), String> {
    let event = PlayerPoolEvent {
        event_type: PlayerPoolEventType::PlayerLeftTeam,
        user_id,
        username,
        league_id,
        team_id: Some(team_id),
        team_name: Some(team_name),
        timestamp: Utc::now(),
    };

    publish_player_pool_event(redis_client, event).await
}
