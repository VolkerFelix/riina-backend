use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;

use crate::{
    db::chat::{
        create_chat_message, get_team_chat_history, get_team_message_count,
        is_active_team_member, edit_chat_message, delete_chat_message,
        admin_delete_chat_message, is_team_admin_or_owner, get_chat_message_with_user,
    },
    middleware::auth::Claims,
    models::chat::{
        SendChatMessageRequest, ChatMessageResponse, ChatHistoryResponse,
        EditChatMessageRequest,
    },
    models::common::ApiResponse,
    services::chat_events,
};

/// Send a chat message to a team
pub async fn send_team_chat_message(
    pool: web::Data<PgPool>,
    team_id: web::Path<Uuid>,
    body: web::Json<SendChatMessageRequest>,
    claims: web::ReqData<Claims>,
    redis_client: web::Data<Arc<redis::Client>>,
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };
    let team_id = team_id.into_inner();

    // Validate the request
    if let Err(e) = body.validate() {
        return HttpResponse::BadRequest().json(
            ChatMessageResponse {
                success: false,
                message: e,
                chat_message: None,
            }
        );
    }

    // Check if user is an active team member
    match is_active_team_member(&pool, user_id, team_id).await {
        Ok(true) => {},
        Ok(false) => {
            return HttpResponse::Forbidden().json(
                ChatMessageResponse {
                    success: false,
                    message: "You are not an active member of this team".to_string(),
                    chat_message: None,
                }
            );
        },
        Err(e) => {
            tracing::error!("Failed to check team membership: {}", e);
            return HttpResponse::InternalServerError().json(
                ChatMessageResponse {
                    success: false,
                    message: "Failed to verify team membership".to_string(),
                    chat_message: None,
                }
            );
        }
    }

    let sanitized_message = body.get_sanitized_message();

    // Create the message
    match create_chat_message(&pool, team_id, user_id, &sanitized_message, body.gif_url.clone(), body.reply_to_message_id).await {
        Ok(chat_message) => {
            // Get the full message info with username and profile picture
            match get_chat_message_with_user(&pool, chat_message.id).await {
                Ok(message_info) => {
                    // Broadcast the message via WebSocket with profile picture and GIF
                    if let Err(e) = chat_events::publish_chat_message(
                        &redis_client,
                        team_id,
                        chat_message.id,
                        user_id,
                        claims.username.clone(),
                        message_info.profile_picture_url.clone(),
                        sanitized_message,
                        message_info.gif_url.clone(),
                    ).await {
                        tracing::warn!("Failed to broadcast chat message: {}", e);
                    }

                    HttpResponse::Ok().json(
                        ChatMessageResponse {
                            success: true,
                            message: "Message sent successfully".to_string(),
                            chat_message: Some(message_info),
                        }
                    )
                },
                Err(e) => {
                    tracing::error!("Failed to fetch created message: {}", e);
                    HttpResponse::InternalServerError().json(
                        ChatMessageResponse {
                            success: false,
                            message: "Message sent but failed to retrieve".to_string(),
                            chat_message: None,
                        }
                    )
                }
            }
        },
        Err(e) => {
            tracing::error!("Failed to create chat message: {}", e);
            HttpResponse::InternalServerError().json(
                ChatMessageResponse {
                    success: false,
                    message: "Failed to send message".to_string(),
                    chat_message: None,
                }
            )
        }
    }
}

/// Get chat history for a team
pub async fn get_team_chat(
    pool: web::Data<PgPool>,
    team_id: web::Path<Uuid>,
    query: web::Query<ChatHistoryQuery>,
    claims: web::ReqData<Claims>,
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };
    let team_id = team_id.into_inner();

    // Check if user is an active team member
    match is_active_team_member(&pool, user_id, team_id).await {
        Ok(true) => {},
        Ok(false) => {
            return HttpResponse::Forbidden().json(
                ChatHistoryResponse {
                    success: false,
                    messages: vec![],
                    total_count: 0,
                    has_more: false,
                }
            );
        },
        Err(e) => {
            tracing::error!("Failed to check team membership: {}", e);
            return HttpResponse::InternalServerError().json(
                ChatHistoryResponse {
                    success: false,
                    messages: vec![],
                    total_count: 0,
                    has_more: false,
                }
            );
        }
    }

    let limit = query.limit.unwrap_or(50).min(100);
    let before_id = query.before.as_ref().and_then(|s| Uuid::parse_str(s).ok());

    // Get messages
    match get_team_chat_history(&pool, team_id, limit, before_id).await {
        Ok(messages) => {
            let total_count = match get_team_message_count(&pool, team_id).await {
                Ok(count) => count,
                Err(_) => 0,
            };

            let has_more = messages.len() as i64 >= limit;

            HttpResponse::Ok().json(
                ChatHistoryResponse {
                    success: true,
                    messages,
                    total_count,
                    has_more,
                }
            )
        },
        Err(e) => {
            tracing::error!("Failed to get chat history: {}", e);
            HttpResponse::InternalServerError().json(
                ChatHistoryResponse {
                    success: false,
                    messages: vec![],
                    total_count: 0,
                    has_more: false,
                }
            )
        }
    }
}

/// Edit a chat message
pub async fn edit_team_chat_message(
    pool: web::Data<PgPool>,
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<EditChatMessageRequest>,
    claims: web::ReqData<Claims>,
    redis_client: web::Data<Arc<redis::Client>>,
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };
    let (team_id, message_id) = path.into_inner();

    // Validate the request
    if let Err(e) = body.validate() {
        return HttpResponse::BadRequest().json(
            ChatMessageResponse {
                success: false,
                message: e,
                chat_message: None,
            }
        );
    }

    // Check if user is an active team member
    match is_active_team_member(&pool, user_id, team_id).await {
        Ok(true) => {},
        Ok(false) => {
            return HttpResponse::Forbidden().json(
                ChatMessageResponse {
                    success: false,
                    message: "You are not an active member of this team".to_string(),
                    chat_message: None,
                }
            );
        },
        Err(e) => {
            tracing::error!("Failed to check team membership: {}", e);
            return HttpResponse::InternalServerError().json(
                ChatMessageResponse {
                    success: false,
                    message: "Failed to verify team membership".to_string(),
                    chat_message: None,
                }
            );
        }
    }

    let sanitized_message = body.get_sanitized_message();

    // Edit the message (only owner can edit)
    match edit_chat_message(&pool, message_id, user_id, &sanitized_message).await {
        Ok(_) => {
            // Broadcast the edit via WebSocket
            if let Err(e) = chat_events::publish_chat_message_edited(
                &redis_client,
                team_id,
                message_id,
                user_id,
                claims.username.clone(),
                sanitized_message,
            ).await {
                tracing::warn!("Failed to broadcast chat message edit: {}", e);
            }

            // Get the updated message
            match get_chat_message_with_user(&pool, message_id).await {
                Ok(message_info) => {
                    HttpResponse::Ok().json(
                        ChatMessageResponse {
                            success: true,
                            message: "Message edited successfully".to_string(),
                            chat_message: Some(message_info),
                        }
                    )
                },
                Err(e) => {
                    tracing::error!("Failed to fetch edited message: {}", e);
                    HttpResponse::InternalServerError().json(
                        ChatMessageResponse {
                            success: false,
                            message: "Message edited but failed to retrieve".to_string(),
                            chat_message: None,
                        }
                    )
                }
            }
        },
        Err(e) => {
            tracing::error!("Failed to edit chat message: {}", e);
            HttpResponse::Forbidden().json(
                ChatMessageResponse {
                    success: false,
                    message: "Failed to edit message (you can only edit your own messages)".to_string(),
                    chat_message: None,
                }
            )
        }
    }
}

/// Delete a chat message (user can delete own messages, admins/owners can delete any)
pub async fn delete_team_chat_message(
    pool: web::Data<PgPool>,
    path: web::Path<(Uuid, Uuid)>,
    claims: web::ReqData<Claims>,
    redis_client: web::Data<Arc<redis::Client>>,
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };
    let (team_id, message_id) = path.into_inner();

    // Check if user is an active team member
    match is_active_team_member(&pool, user_id, team_id).await {
        Ok(true) => {},
        Ok(false) => {
            return HttpResponse::Forbidden().json(
                ChatMessageResponse {
                    success: false,
                    message: "You are not an active member of this team".to_string(),
                    chat_message: None,
                }
            );
        },
        Err(e) => {
            tracing::error!("Failed to check team membership: {}", e);
            return HttpResponse::InternalServerError().json(
                ChatMessageResponse {
                    success: false,
                    message: "Failed to verify team membership".to_string(),
                    chat_message: None,
                }
            );
        }
    }

    // Try to delete as owner first
    let deleted = match delete_chat_message(&pool, message_id, user_id).await {
        Ok(true) => true,
        Ok(false) => {
            // User doesn't own the message, check if they're admin/owner
            match is_team_admin_or_owner(&pool, user_id, team_id).await {
                Ok(true) => {
                    match admin_delete_chat_message(&pool, message_id, team_id).await {
                        Ok(success) => success,
                        Err(e) => {
                            tracing::error!("Failed to admin delete message: {}", e);
                            return HttpResponse::InternalServerError().json(
                                ChatMessageResponse {
                                    success: false,
                                    message: "Failed to delete message".to_string(),
                                    chat_message: None,
                                }
                            );
                        }
                    }
                },
                Ok(false) => {
                    return HttpResponse::Forbidden().json(
                        ChatMessageResponse {
                            success: false,
                            message: "You can only delete your own messages".to_string(),
                            chat_message: None,
                        }
                    );
                },
                Err(e) => {
                    tracing::error!("Failed to check admin status: {}", e);
                    return HttpResponse::InternalServerError().json(
                        ChatMessageResponse {
                            success: false,
                            message: "Failed to verify permissions".to_string(),
                            chat_message: None,
                        }
                    );
                }
            }
        },
        Err(e) => {
            tracing::error!("Failed to delete message: {}", e);
            return HttpResponse::InternalServerError().json(
                ChatMessageResponse {
                    success: false,
                    message: "Failed to delete message".to_string(),
                    chat_message: None,
                }
            );
        }
    };

    if deleted {
        // Broadcast the deletion via WebSocket
        if let Err(e) = chat_events::publish_chat_message_deleted(
            &redis_client,
            team_id,
            message_id,
            user_id,
        ).await {
            tracing::warn!("Failed to broadcast chat message deletion: {}", e);
        }

        HttpResponse::Ok().json(
            ChatMessageResponse {
                success: true,
                message: "Message deleted successfully".to_string(),
                chat_message: None,
            }
        )
    } else {
        HttpResponse::NotFound().json(
            ChatMessageResponse {
                success: false,
                message: "Message not found".to_string(),
                chat_message: None,
            }
        )
    }
}

#[derive(serde::Deserialize)]
pub struct ChatHistoryQuery {
    pub limit: Option<i64>,
    pub before: Option<String>, // Message ID to fetch messages before
}
