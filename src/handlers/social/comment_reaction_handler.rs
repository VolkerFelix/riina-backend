use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;

use crate::{
    db::social::{
        create_comment_reaction, delete_comment_reaction, get_comment_reactions, get_comment_reaction_users,
    },
    middleware::auth::Claims,
    models::social::{CreateCommentReactionRequest, ReactionType},
    models::common::ApiResponse,
    services::social_events,
};

pub async fn add_comment_reaction(
    pool: web::Data<PgPool>,
    comment_id: web::Path<Uuid>,
    body: web::Json<CreateCommentReactionRequest>,
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

    if ReactionType::from_str(&body.reaction_type).is_none() {
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error("Invalid reaction type")
        );
    }

    match create_comment_reaction(&pool, user_id, comment_id, &body.reaction_type).await {
        Ok(reaction) => {
            // Broadcast WebSocket event (fire and forget)
            if let Err(e) = social_events::broadcast_comment_reaction_added(
                &redis_client,
                comment_id,
                user_id,
                claims.username.clone(),
                body.reaction_type.clone(),
            ).await {
                tracing::warn!("Failed to broadcast comment reaction added event: {}", e);
            }

            HttpResponse::Ok().json(reaction)
        }
        Err(e) => {
            tracing::error!("Failed to create comment reaction: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to create comment reaction")
            )
        }
    }
}

pub async fn remove_comment_reaction(
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

    match delete_comment_reaction(&pool, user_id, comment_id).await {
        Ok(deleted) => {
            if deleted {
                // Broadcast WebSocket event (fire and forget)
                if let Err(e) = social_events::broadcast_comment_reaction_removed(
                    &redis_client,
                    comment_id,
                    user_id,
                    claims.username.clone(),
                ).await {
                    tracing::warn!("Failed to broadcast comment reaction removed event: {}", e);
                }

                HttpResponse::Ok().json(serde_json::json!({
                    "message": "Reaction removed successfully"
                }))
            } else {
                HttpResponse::NotFound().json(
                    ApiResponse::<()>::error("Reaction not found")
                )
            }
        }
        Err(e) => {
            tracing::error!("Failed to delete comment reaction: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to delete comment reaction")
            )
        }
    }
}

pub async fn get_comment_reactions_handler(
    pool: web::Data<PgPool>,
    comment_id: web::Path<Uuid>,
    claims: web::ReqData<Claims>,
) -> HttpResponse {
    let comment_id = comment_id.into_inner();
    let current_user_id = Uuid::parse_str(&claims.sub.clone()).ok();

    match get_comment_reactions(&pool, comment_id, current_user_id).await {
        Ok(reactions) => HttpResponse::Ok().json(reactions),
        Err(e) => {
            tracing::error!("Failed to get comment reactions: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to get comment reactions")
            )
        }
    }
}

pub async fn get_comment_reaction_details(
    pool: web::Data<PgPool>,
    comment_id: web::Path<Uuid>,
    query: web::Query<std::collections::HashMap<String, String>>,
    _claims: web::ReqData<Claims>,
) -> HttpResponse {
    let comment_id = comment_id.into_inner();
    let reaction_type = query.get("type").map(|s| s.as_str());

    match get_comment_reaction_users(&pool, comment_id, reaction_type).await {
        Ok(users) => HttpResponse::Ok().json(users),
        Err(e) => {
            tracing::error!("Failed to get comment reaction users: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to get comment reaction users")
            )
        }
    }
}
