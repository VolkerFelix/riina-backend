use redis::AsyncCommands;
use chrono::Utc;
use uuid::Uuid;
use tracing;
use actix_web::web;
use std::sync::Arc;

use crate::models::game_events::GameEvent;

/// Broadcast workout reaction added event via Redis
pub async fn broadcast_reaction_added(
    redis_client: &web::Data<Arc<redis::Client>>,
    workout_id: Uuid,
    user_id: Uuid,
    username: String,
    reaction_type: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let event = GameEvent::WorkoutReactionAdded {
        workout_id,
        user_id,
        username: username.clone(),
        reaction_type: reaction_type.clone(),
        timestamp: Utc::now(),
    };

    broadcast_event(redis_client, &event).await?;
    tracing::info!("📢 Broadcasted reaction added: {} reacted with {} to workout {}", username, reaction_type, workout_id);
    Ok(())
}

/// Broadcast workout reaction removed event via Redis
pub async fn broadcast_reaction_removed(
    redis_client: &web::Data<Arc<redis::Client>>,
    workout_id: Uuid,
    user_id: Uuid,
    username: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let event = GameEvent::WorkoutReactionRemoved {
        workout_id,
        user_id,
        username: username.clone(),
        timestamp: Utc::now(),
    };

    broadcast_event(redis_client, &event).await?;
    tracing::info!("📢 Broadcasted reaction removed: {} removed reaction from workout {}", username, workout_id);
    Ok(())
}

/// Broadcast workout comment added event via Redis
pub async fn broadcast_comment_added(
    redis_client: &web::Data<Arc<redis::Client>>,
    workout_id: Uuid,
    comment_id: Uuid,
    user_id: Uuid,
    username: String,
    content: String,
    parent_id: Option<Uuid>,
) -> Result<(), Box<dyn std::error::Error>> {
    let event = GameEvent::WorkoutCommentAdded {
        workout_id,
        comment_id,
        user_id,
        username: username.clone(),
        content: content.clone(),
        parent_id,
        timestamp: Utc::now(),
    };

    broadcast_event(redis_client, &event).await?;
    tracing::info!("📢 Broadcasted comment added: {} commented on workout {} ({})",
        username, workout_id, if parent_id.is_some() { "reply" } else { "comment" });
    Ok(())
}

/// Broadcast workout comment updated event via Redis
pub async fn broadcast_comment_updated(
    redis_client: &web::Data<Arc<redis::Client>>,
    workout_id: Uuid,
    comment_id: Uuid,
    user_id: Uuid,
    username: String,
    content: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let event = GameEvent::WorkoutCommentUpdated {
        workout_id,
        comment_id,
        user_id,
        username: username.clone(),
        content: content.clone(),
        timestamp: Utc::now(),
    };

    broadcast_event(redis_client, &event).await?;
    tracing::info!("📢 Broadcasted comment updated: {} edited comment {} on workout {}", username, comment_id, workout_id);
    Ok(())
}

/// Broadcast workout comment deleted event via Redis
pub async fn broadcast_comment_deleted(
    redis_client: &web::Data<Arc<redis::Client>>,
    workout_id: Uuid,
    comment_id: Uuid,
    user_id: Uuid,
    username: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let event = GameEvent::WorkoutCommentDeleted {
        workout_id,
        comment_id,
        user_id,
        username: username.clone(),
        timestamp: Utc::now(),
    };

    broadcast_event(redis_client, &event).await?;
    tracing::info!("📢 Broadcasted comment deleted: {} deleted comment {} from workout {}", username, comment_id, workout_id);
    Ok(())
}

/// Helper function to broadcast any GameEvent to the global channel
async fn broadcast_event(
    redis_client: &web::Data<Arc<redis::Client>>,
    event: &GameEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = redis_client.get_async_connection().await?;
    let message = serde_json::to_string(event)?;

    let global_channel = "game:events:global";
    let result: Result<i32, redis::RedisError> = conn.publish(global_channel, message).await;

    match result {
        Ok(subscriber_count) => {
            tracing::debug!("✅ Social event broadcasted to {} subscribers on {}", subscriber_count, global_channel);
            Ok(())
        },
        Err(e) => {
            tracing::error!("❌ Failed to broadcast social event to global channel: {}", e);
            Err(Box::new(e))
        }
    }
}