use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    db::social::{get_notifications, mark_notification_read, mark_all_notifications_read, get_unread_count},
    middleware::auth::Claims,
    models::social::NotificationQueryParams,
    models::common::ApiResponse,
};

pub async fn get_user_notifications(
    pool: web::Data<PgPool>,
    query: web::Query<NotificationQueryParams>,
    claims: web::ReqData<Claims>,
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

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let unread_only = query.unread_only.unwrap_or(false);

    match get_notifications(&pool, user_id, page, per_page, unread_only).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            tracing::error!("Failed to get notifications: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to get notifications")
            )
        }
    }
}

pub async fn mark_notification_as_read(
    pool: web::Data<PgPool>,
    notification_id: web::Path<Uuid>,
    claims: web::ReqData<Claims>,
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
    let notification_id = notification_id.into_inner();

    match mark_notification_read(&pool, notification_id, user_id).await {
        Ok(updated) => {
            if updated {
                HttpResponse::Ok().json(ApiResponse::<()>::success("Notification marked as read", ()))
            } else {
                HttpResponse::NotFound().json(
                    ApiResponse::<()>::error("Notification not found")
                )
            }
        }
        Err(e) => {
            tracing::error!("Failed to mark notification as read: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to mark notification as read")
            )
        }
    }
}

pub async fn mark_all_as_read(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
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

    match mark_all_notifications_read(&pool, user_id).await {
        Ok(count) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "marked_read": count
        })),
        Err(e) => {
            tracing::error!("Failed to mark all notifications as read: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to mark all notifications as read")
            )
        }
    }
}

pub async fn get_unread_notification_count(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
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

    match get_unread_count(&pool, user_id).await {
        Ok(count) => HttpResponse::Ok().json(serde_json::json!({
            "unread_count": count
        })),
        Err(e) => {
            tracing::error!("Failed to get unread count: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to get unread count")
            )
        }
    }
}
