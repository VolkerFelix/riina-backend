use chrono::Utc;
use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;
use redis::Client as RedisClient;

use crate::models::player_pool::{PlayerPoolEvent, PlayerPoolEventType};

/// Publish a player pool event to Redis for WebSocket broadcasting AND save to database
pub async fn publish_player_pool_event(
    redis_client: &Arc<RedisClient>,
    pool: &PgPool,
    event: PlayerPoolEvent,
) -> Result<(), String> {
    // 1. Save to database as a broadcast notification
    let message = match event.event_type {
        PlayerPoolEventType::PlayerJoined => {
            format!("{} is now available as a free agent", event.username)
        }
        PlayerPoolEventType::PlayerLeft => {
            format!("{} is no longer available", event.username)
        }
        PlayerPoolEventType::PlayerAssigned => {
            format!("{} joined {}", event.username, event.team_name.as_deref().unwrap_or("a team"))
        }
        PlayerPoolEventType::PlayerLeftTeam => {
            format!("{} left {} and is back in the free agent pool",
                event.username, event.team_name.as_deref().unwrap_or("their team"))
        }
    };

    sqlx::query!(
        r#"
        INSERT INTO notifications (recipient_id, actor_id, notification_type, entity_type, entity_id, message, read, created_at)
        VALUES (NULL, $1, 'player_pool_event', 'player_pool', $2, $3, false, $4)
        "#,
        event.user_id,
        event.user_id,
        message,
        event.timestamp
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to save player pool notification to database: {}", e))?;

    // 2. Publish to Redis for real-time WebSocket broadcasting
    let mut conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("Failed to get Redis connection: {}", e))?;

    let channel = "player_pool_events";
    let redis_message = serde_json::to_string(&event)
        .map_err(|e| format!("Failed to serialize player pool event: {}", e))?;

    conn.publish::<_, _, ()>(channel, redis_message)
        .await
        .map_err(|e| format!("Failed to publish player pool event to Redis: {}", e))?;

    tracing::info!(
        "Published and saved player pool event: {} for user {}",
        event.event_type.as_str(),
        event.user_id
    );

    Ok(())
}

/// Publish a "player joined pool" event
pub async fn publish_player_joined(
    redis_client: &Arc<RedisClient>,
    pool: &PgPool,
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

    publish_player_pool_event(redis_client, pool, event).await
}

/// Publish a "player left pool" event (user went inactive)
pub async fn publish_player_left(
    redis_client: &Arc<RedisClient>,
    pool: &PgPool,
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

    publish_player_pool_event(redis_client, pool, event).await
}

/// Publish a "player assigned to team" event
pub async fn publish_player_assigned(
    redis_client: &Arc<RedisClient>,
    pool: &PgPool,
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

    publish_player_pool_event(redis_client, pool, event).await
}

/// Publish a "player left team" event
pub async fn publish_player_left_team(
    redis_client: &Arc<RedisClient>,
    pool: &PgPool,
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

    publish_player_pool_event(redis_client, pool, event).await
}
