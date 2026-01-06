use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;

use crate::{
    db::social::{
        create_comment, delete_comment, get_comment_by_id, get_workout_comments, update_comment,
        create_notification, get_workout_owner,
    },
    middleware::auth::Claims,
    models::social::{CommentQueryParams, CreateCommentRequest, UpdateCommentRequest, NotificationType},
    models::common::ApiResponse,
    services::social_events,
    handlers::notification_handler::send_notification_to_user
};

pub async fn add_comment(
    pool: web::Data<PgPool>,
    workout_id: web::Path<Uuid>,
    body: web::Json<CreateCommentRequest>,
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

    if body.content.trim().is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Comment content cannot be empty"
        }));
    }

    if body.content.len() > 1000 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Comment content too long (max 1000 characters)"
        }));
    }

    match create_comment(&pool, user_id, workout_id, &body.content, body.parent_id).await {
        Ok(comment) => {
            // Create notification
            if let Some(parent_id) = body.parent_id {
                // This is a reply - notify the parent comment author
                if let Ok(Some(parent_comment)) = get_comment_by_id(&pool, parent_id).await {
                    let message = format!("{} replied to your comment", claims.username);
                    match create_notification(
                        &pool,
                        parent_comment.user_id,
                        user_id,
                        NotificationType::Reply.as_str(),
                        "comment",
                        comment.id,
                        &message,
                    ).await {
                        Ok(Some(notification_id)) => {
                            // Broadcast notification via WebSocket
                            if let Err(e) = social_events::send_websocket_notification_to_user(
                                &redis_client,
                                parent_comment.user_id,
                                notification_id,
                                claims.username.clone(),
                                NotificationType::Reply.as_str().to_string(),
                                message.clone(),
                            ).await {
                                tracing::warn!("Failed to broadcast notification: {}", e);
                            }

                            // Send push notification
                            let notification_data = serde_json::json!({
                                "type": "reply",
                                "comment_id": comment.id.to_string(),
                                "parent_id": parent_id.to_string(),
                                "workout_id": workout_id.to_string(),
                            });

                            let comment_preview = if body.content.len() > 50 {
                                format!("{}...", &body.content[..50])
                            } else {
                                body.content.clone()
                            };

                            if let Err(e) = send_notification_to_user(
                                &pool,
                                parent_comment.user_id,
                                format!("{} replied to your comment", claims.username),
                                comment_preview,
                                Some(notification_data),
                                Some("reply".to_string())
                            ).await {
                                tracing::warn!("Failed to send push notification: {}", e);
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!("Failed to create notification: {}", e);
                        }
                    }
                }
            } else {
                // This is a top-level comment - notify the workout owner
                if let Ok(Some(workout_owner_id)) = get_workout_owner(&pool, workout_id).await {
                    let message = format!("{} commented on your workout", claims.username);
                    match create_notification(
                        &pool,
                        workout_owner_id,
                        user_id,
                        NotificationType::Comment.as_str(),
                        "post",
                        comment.id,
                        &message,
                    ).await {
                        Ok(Some(notification_id)) => {
                            // Broadcast notification via WebSocket
                            if let Err(e) = social_events::send_websocket_notification_to_user(
                                &redis_client,
                                workout_owner_id,
                                notification_id,
                                claims.username.clone(),
                                NotificationType::Comment.as_str().to_string(),
                                message.clone(),
                            ).await {
                                tracing::warn!("Failed to broadcast notification: {}", e);
                            }

                            // Send push notification
                            let notification_data = serde_json::json!({
                                "type": "comment",
                                "comment_id": comment.id.to_string(),
                                "workout_id": workout_id.to_string(),
                            });

                            let comment_preview = if body.content.len() > 50 {
                                format!("{}...", &body.content[..50])
                            } else {
                                body.content.clone()
                            };

                            if let Err(e) = send_notification_to_user(
                                &pool,
                                workout_owner_id,
                                format!("{} commented on your workout", claims.username),
                                comment_preview,
                                Some(notification_data),
                                Some("comment".to_string())
                            ).await {
                                tracing::warn!("Failed to send push notification: {}", e);
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!("Failed to create notification: {}", e);
                        }
                    }
                }
            }

            // Broadcast WebSocket event (fire and forget)
            if let Err(e) = social_events::broadcast_comment_added(
                &redis_client,
                workout_id,
                comment.id,
                user_id,
                claims.username.clone(),
                body.content.clone(),
                body.parent_id,
            ).await {
                tracing::warn!("Failed to broadcast comment added event: {}", e);
            }

            HttpResponse::Ok().json(comment)
        }
        Err(e) => {
            tracing::error!("Failed to create comment: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to create comment"
            }))
        }
    }
}

pub async fn edit_comment(
    pool: web::Data<PgPool>,
    comment_id: web::Path<Uuid>,
    body: web::Json<UpdateCommentRequest>,
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
    let comment_id = comment_id.into_inner();

    if body.content.trim().is_empty() {
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error("Comment content cannot be empty")
        );
    }

    if body.content.len() > 1000 {
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error("Comment content too long (max 1000 characters)")
        );
    }

    // First check if the comment exists
    let existing_comment = match get_comment_by_id(&pool, comment_id).await {
        Ok(Some(comment)) => comment,
        Ok(None) => {
            return HttpResponse::NotFound().json(
                ApiResponse::<()>::error("Comment not found")
            );
        }
        Err(e) => {
            tracing::error!("Failed to get comment: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to get comment")
            );
        }
    };

    // Check if the user owns the comment
    if existing_comment.user_id != user_id {
        return HttpResponse::Forbidden().json(
            ApiResponse::<()>::error("You don't have permission to edit this comment")
        );
    }

    match update_comment(&pool, comment_id, user_id, &body.content).await {
        Ok(Some(comment)) => {
            // Broadcast WebSocket event (fire and forget)
            if let Err(e) = social_events::broadcast_comment_updated(
                &redis_client,
                comment.workout_id,
                comment_id,
                user_id,
                claims.username.clone(),
                body.content.clone(),
            ).await {
                tracing::warn!("Failed to broadcast comment updated event: {}", e);
            }

            HttpResponse::Ok().json(comment)
        }
        Ok(None) => HttpResponse::NotFound().json(
            ApiResponse::<()>::error("Comment not found")
        ),
        Err(e) => {
            tracing::error!("Failed to update comment: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to update comment")
            )
        }
    }
}

pub async fn remove_comment(
    pool: web::Data<PgPool>,
    comment_id: web::Path<Uuid>,
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
    let comment_id = comment_id.into_inner();

    // Get comment info before deletion for WebSocket event
    let comment_info = get_comment_by_id(&pool, comment_id).await;

    match delete_comment(&pool, comment_id, user_id).await {
        Ok(deleted) => {
            if deleted {
                // Broadcast WebSocket event if we have comment info (fire and forget)
                if let Ok(Some(comment)) = comment_info {
                    if let Err(e) = social_events::broadcast_comment_deleted(
                        &redis_client,
                        comment.workout_id,
                        comment_id,
                        user_id,
                        claims.username.clone(),
                    ).await {
                        tracing::warn!("Failed to broadcast comment deleted event: {}", e);
                    }
                }

                HttpResponse::Ok().json(serde_json::json!({
                    "success": true
                }))
            } else {
                HttpResponse::NotFound().json(serde_json::json!({
                    "error": "Comment not found or you don't have permission to delete it"
                }))
            }
        }
        Err(e) => {
            tracing::error!("Failed to delete comment: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to delete comment"
            }))
        }
    }
}

pub async fn get_comments(
    pool: web::Data<PgPool>,
    workout_id: web::Path<Uuid>,
    query: web::Query<CommentQueryParams>,
    claims: web::ReqData<Claims>,
) -> HttpResponse {
    let workout_id = workout_id.into_inner();
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let current_user_id = Uuid::parse_str(&claims.sub.clone()).ok();

    match get_workout_comments(&pool, workout_id, page, per_page, current_user_id).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            tracing::error!("Failed to get comments: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get comments"
            }))
        }
    }
}

pub async fn get_single_comment(
    pool: web::Data<PgPool>,
    comment_id: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> HttpResponse {
    let comment_id = comment_id.into_inner();

    match get_comment_by_id(&pool, comment_id).await {
        Ok(Some(comment)) => HttpResponse::Ok().json(comment),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Comment not found"
        })),
        Err(e) => {
            tracing::error!("Failed to get comment: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get comment"
            }))
        }
    }
}