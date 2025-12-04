use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;

use crate::{
    db::social::{create_reaction, delete_reaction, get_workout_reactions, get_reaction_users, create_notification, get_workout_owner},
    middleware::auth::Claims,
    models::social::{CreateReactionRequest, ReactionType, NotificationType},
    models::common::ApiResponse,
    services::social_events,
};


pub async fn add_reaction(
    pool: web::Data<PgPool>,
    workout_id: web::Path<Uuid>,
    body: web::Json<CreateReactionRequest>,
    claims: web::ReqData<Claims>,
    redis_client: web::Data<Arc<redis::Client>>,
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub.clone()) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };
    let workout_id = workout_id.into_inner();

    if ReactionType::from_str(&body.reaction_type).is_none() {
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error("Invalid reaction type")
        );
    }

    match create_reaction(&pool, user_id, workout_id, &body.reaction_type).await {
        Ok(reaction) => {
            // Get workout owner to send notification
            if let Ok(Some(workout_owner_id)) = get_workout_owner(&pool, workout_id).await {
                // Don't send notification if user reacted to their own workout
                if workout_owner_id == user_id {
                    // User reacted to their own workout, skip notification
                } else {
                    let message = format!("{} reacted to your workout", claims.username);

                    // Create in-app notification FIRST
                    match create_notification(
                        &pool,
                        workout_owner_id,
                        user_id,
                        NotificationType::Reaction.as_str(),
                        "workout",
                        workout_id,
                        &message,
                    ).await {
                        Ok(Some(notification_id)) => {
                            // Broadcast notification via WebSocket
                            if let Err(e) = social_events::send_websocket_notification_to_user(
                                &redis_client,
                                workout_owner_id,
                                notification_id,
                                claims.username.clone(),
                                NotificationType::Reaction.as_str().to_string(),
                                message.clone(),
                            ).await {
                                tracing::warn!("Failed to broadcast notification: {}", e);
                            }

                            // Now send push notification with accurate badge count
                            let notification_data = serde_json::json!({
                                "type": "reaction",
                                "workout_id": workout_id.to_string(),
                                "notification_id": notification_id.to_string(),
                            });

                            if let Err(e) = crate::handlers::notification_handler::send_notification_to_user(
                                &pool,
                                workout_owner_id,
                                format!("{} reacted to your workout", claims.username),
                                format!("🔥"),
                                Some(notification_data),
                                Some("reaction".to_string())
                            ).await {
                                tracing::warn!("Failed to send push notification: {}", e);
                            }
                        }
                        Ok(None) => {
                            // User reacted to their own workout, no notification needed
                        }
                        Err(e) => {
                            tracing::warn!("Failed to create notification: {}", e);
                        }
                    }
                }
            }

            // Broadcast WebSocket event (fire and forget)
            if let Err(e) = social_events::broadcast_reaction_added(
                &redis_client,
                workout_id,
                user_id,
                claims.username.clone(),
                body.reaction_type.clone(),
            ).await {
                tracing::warn!("Failed to broadcast reaction added event: {}", e);
            }

            HttpResponse::Ok().json(reaction)
        }
        Err(e) => {
            tracing::error!("Failed to create reaction: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to create reaction")
            )
        }
    }
}

pub async fn remove_reaction(
    pool: web::Data<PgPool>,
    workout_id: web::Path<Uuid>,
    claims: web::ReqData<Claims>,
    redis_client: web::Data<Arc<redis::Client>>,
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub.clone()) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };
    let workout_id = workout_id.into_inner();

    match delete_reaction(&pool, user_id, workout_id).await {
        Ok(deleted) => {
            if deleted {
                // Broadcast WebSocket event (fire and forget)
                if let Err(e) = social_events::broadcast_reaction_removed(
                    &redis_client,
                    workout_id,
                    user_id,
                    claims.username.clone(),
                ).await {
                    tracing::warn!("Failed to broadcast reaction removed event: {}", e);
                }

                HttpResponse::Ok().json(ApiResponse::<()>::success("Reaction removed successfully", ()))
            } else {
                HttpResponse::NotFound().json(
                    ApiResponse::<()>::error("Reaction not found")
                )
            }
        }
        Err(e) => {
            tracing::error!("Failed to delete reaction: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to delete reaction")
            )
        }
    }
}

pub async fn get_reactions(
    pool: web::Data<PgPool>,
    workout_id: web::Path<Uuid>,
    claims: web::ReqData<Claims>,
) -> HttpResponse {
    let workout_id = workout_id.into_inner();
    let current_user_id = Uuid::parse_str(&claims.sub.clone()).ok();

    match get_workout_reactions(&pool, workout_id, current_user_id).await {
        Ok(reactions) => HttpResponse::Ok().json(reactions),
        Err(e) => {
            tracing::error!("Failed to get reactions: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to get reactions")
            )
        }
    }
}

pub async fn get_reaction_details(
    pool: web::Data<PgPool>,
    workout_id: web::Path<Uuid>,
    query: web::Query<std::collections::HashMap<String, String>>,
    _claims: web::ReqData<Claims>,
) -> HttpResponse {
    let workout_id = workout_id.into_inner();
    let reaction_type = query.get("type").map(|s| s.as_str());

    match get_reaction_users(&pool, workout_id, reaction_type).await {
        Ok(users) => HttpResponse::Ok().json(users),
        Err(e) => {
            tracing::error!("Failed to get reaction users: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to get reaction users")
            )
        }
    }
}