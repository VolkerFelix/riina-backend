use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;

use crate::{
    db::social::{
        create_comment, delete_comment, get_comment_by_id, get_workout_comments, update_comment,
    },
    middleware::auth::Claims,
    models::social::{CommentQueryParams, CreateCommentRequest, UpdateCommentRequest},
    models::common::ApiResponse,
    services::social_events,
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
            ApiResponse::<()>::error("Comment not found or you don't have permission to edit it")
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