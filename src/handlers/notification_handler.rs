use actix_web::{web, HttpResponse};
use serde_json::json;
use sqlx::PgPool;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::middleware::auth::Claims;
use crate::models::notification::{
    RegisterPushTokenRequest, UnregisterPushTokenRequest, SendNotificationRequest,
    PushToken, PushTokenResponse, ExpoPushMessage, SendNotificationResponse,
};

/// Register a push notification token for the authenticated user
pub async fn register_push_token(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    req: web::Json<RegisterPushTokenRequest>,
) -> actix_web::Result<HttpResponse> {
    let user_id = Uuid::parse_str(&claims.sub).map_err(|e| {
        error!("Failed to parse user_id from claims: {}", e);
        actix_web::error::ErrorInternalServerError("Invalid user ID")
    })?;

    info!("Registering push token for user_id={} platform={}", user_id, req.platform);

    // Validate platform
    if !["ios", "android", "web"].contains(&req.platform.as_str()) {
        warn!("Invalid platform attempted: {}", req.platform);
        return Ok(HttpResponse::BadRequest().json(json!({
            "error": "Invalid platform. Must be one of: ios, android, web"
        })));
    }

    // Check if token already exists
    let existing_token = sqlx::query_as::<_, PushToken>(
        "SELECT * FROM push_tokens WHERE token = $1"
    )
    .bind(&req.token)
    .fetch_optional(pool.as_ref())
    .await
    .map_err(|e| {
        error!("Database error checking existing token: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    let token = if let Some(mut existing) = existing_token {
        // Update existing token
        info!("Updating existing push token for user_id={}", user_id);
        existing.user_id = user_id;
        existing.platform = req.platform.clone();
        existing.device_info = req.device_info.clone();
        existing.is_active = true;

        sqlx::query_as::<_, PushToken>(
            "UPDATE push_tokens
             SET user_id = $1, platform = $2, device_info = $3, is_active = $4, last_used_at = NOW()
             WHERE token = $5
             RETURNING *"
        )
        .bind(user_id)
        .bind(&req.platform)
        .bind(&req.device_info)
        .bind(true)
        .bind(&req.token)
        .fetch_one(pool.as_ref())
        .await
        .map_err(|e| {
            error!("Database error updating token: {}", e);
            actix_web::error::ErrorInternalServerError("Database error")
        })?
    } else {
        // Insert new token
        info!("Inserting new push token for user_id={}", user_id);
        sqlx::query_as::<_, PushToken>(
            "INSERT INTO push_tokens (user_id, token, platform, device_info, is_active, last_used_at)
             VALUES ($1, $2, $3, $4, $5, NOW())
             RETURNING *"
        )
        .bind(user_id)
        .bind(&req.token)
        .bind(&req.platform)
        .bind(&req.device_info)
        .bind(true)
        .fetch_one(pool.as_ref())
        .await
        .map_err(|e| {
            error!("Database error inserting token: {}", e);
            actix_web::error::ErrorInternalServerError("Database error")
        })?
    };

    info!("Successfully registered push token for user_id={}", user_id);
    let response: PushTokenResponse = token.into();
    Ok(HttpResponse::Ok().json(response))
}

/// Unregister a push notification token
pub async fn unregister_push_token(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    req: web::Json<UnregisterPushTokenRequest>,
) -> actix_web::Result<HttpResponse> {
    let user_id = Uuid::parse_str(&claims.sub).map_err(|e| {
        error!("Failed to parse user_id from claims: {}", e);
        actix_web::error::ErrorInternalServerError("Invalid user ID")
    })?;

    info!("Unregistering push token for user_id={}", user_id);

    sqlx::query(
        "UPDATE push_tokens SET is_active = false WHERE user_id = $1 AND token = $2"
    )
    .bind(user_id)
    .bind(&req.token)
    .execute(pool.as_ref())
    .await
    .map_err(|e| {
        error!("Database error unregistering token: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    info!("Successfully unregistered push token for user_id={}", user_id);
    Ok(HttpResponse::Ok().json(json!({
        "message": "Token unregistered successfully"
    })))
}

/// Get all active push tokens for the authenticated user
pub async fn get_user_tokens(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> actix_web::Result<HttpResponse> {
    let user_id = Uuid::parse_str(&claims.sub).map_err(|e| {
        error!("Failed to parse user_id from claims: {}", e);
        actix_web::error::ErrorInternalServerError("Invalid user ID")
    })?;

    info!("Fetching push tokens for user_id={}", user_id);

    let tokens = sqlx::query_as::<_, PushToken>(
        "SELECT * FROM push_tokens WHERE user_id = $1 AND is_active = true ORDER BY created_at DESC"
    )
    .bind(user_id)
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| {
        error!("Database error fetching tokens: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    info!("Found {} active tokens for user_id={}", tokens.len(), user_id);
    let response: Vec<PushTokenResponse> = tokens.into_iter().map(|t| t.into()).collect();
    Ok(HttpResponse::Ok().json(response))
}

/// Send push notifications to specific users (admin/system use)
pub async fn send_notification(
    pool: web::Data<PgPool>,
    req: web::Json<SendNotificationRequest>,
) -> actix_web::Result<HttpResponse> {
    info!(
        "Sending notification to {} users: {}",
        req.user_ids.len(),
        req.title
    );

    // Fetch all active tokens for the specified users
    let tokens = sqlx::query_as::<_, PushToken>(
        "SELECT * FROM push_tokens
         WHERE user_id = ANY($1) AND is_active = true"
    )
    .bind(&req.user_ids)
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| {
        error!("Database error fetching tokens: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    if tokens.is_empty() {
        warn!("No active tokens found for the specified users");
        return Ok(HttpResponse::Ok().json(SendNotificationResponse {
            success: true,
            sent_count: 0,
            failed_count: 0,
        }));
    }

    info!("Found {} active tokens for notification", tokens.len());

    // Build notification messages with badge counts
    let mut messages: Vec<ExpoPushMessage> = Vec::new();

    for token in tokens {
        let mut message = ExpoPushMessage::new(
            token.token.clone(),
            req.title.clone(),
            req.body.clone(),
        );

        if let Some(data) = &req.data {
            message = message.with_data(data.clone());
        }

        if let Some(notification_type) = &req.notification_type {
            let channel = match notification_type.as_str() {
                "league_update" => "league_updates",
                "game_event" => "game_events",
                "health_reminder" => "health_reminders",
                "team_message" => "team_messages",
                _ => "default",
            };
            message = message.with_channel(channel.to_string());
        }

        // Calculate badge count for this user
        let notification_count = match crate::db::social::get_unread_count(&pool, token.user_id).await {
            Ok(count) => count,
            Err(e) => {
                error!("Failed to get unread notification count for user {}: {}", token.user_id, e);
                0
            }
        };

        let message_count = match crate::db::chat::get_unread_message_count(&pool, token.user_id).await {
            Ok(count) => count,
            Err(e) => {
                error!("Failed to get unread message count for user {}: {}", token.user_id, e);
                0
            }
        };

        let badge_count = (notification_count + message_count) as i32;
        message = message.with_badge(badge_count);

        messages.push(message);
    }

    // Send to Expo Push Notification service
    info!("Sending {} messages to Expo Push Service", messages.len());
    let client = reqwest::Client::new();
    let response = client
        .post("https://exp.host/--/api/v2/push/send")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .json(&messages)
        .send()
        .await
        .map_err(|e| {
            error!("Error sending push notifications: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to send notifications")
        })?;

    if response.status().is_success() {
        // Parse and log Expo's response to check for errors in tickets
        let response_text = response.text().await.unwrap_or_else(|_| "{}".to_string());

        // Try to parse the response to check for errors
        if let Ok(expo_response) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if let Some(data) = expo_response.get("data").and_then(|d| d.as_array()) {
                for (idx, ticket) in data.iter().enumerate() {
                    if let Some(status) = ticket.get("status").and_then(|s| s.as_str()) {
                        if status == "error" {
                            let message = ticket.get("message").and_then(|m| m.as_str()).unwrap_or("unknown");
                            let details = ticket.get("details").and_then(|d| d.as_str()).unwrap_or("");
                            error!("Expo push ticket {} error: {} - {}", idx, message, details);
                        }
                    }
                }
            }
        }

        info!("Successfully sent {} push notifications", messages.len());
        Ok(HttpResponse::Ok().json(SendNotificationResponse {
            success: true,
            sent_count: messages.len(),
            failed_count: 0,
        }))
    } else {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        error!("Expo push service error: {}", error_text);
        Ok(HttpResponse::InternalServerError().json(json!({
            "error": "Failed to send notifications",
            "details": error_text
        })))
    }
}

/// Helper function to send notifications to a single user
pub async fn send_notification_to_user(
    pool: &PgPool,
    user_id: Uuid,
    title: String,
    body: String,
    data: Option<serde_json::Value>,
    notification_type: Option<String>,
) -> Result<(), String> {
    info!("Sending notification to single user {}: {}", user_id, title);

    let req = SendNotificationRequest {
        user_ids: vec![user_id],
        title,
        body,
        data,
        notification_type,
    };

    // Fetch tokens
    let tokens = sqlx::query_as::<_, PushToken>(
        "SELECT * FROM push_tokens WHERE user_id = $1 AND is_active = true"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("Database error fetching tokens for user {}: {}", user_id, e);
        format!("Database error: {}", e)
    })?;

    if tokens.is_empty() {
        warn!("No active tokens found for user {}", user_id);
        return Ok(()); // No tokens to send to
    }

    info!("Found {} tokens for user {}", tokens.len(), user_id);

    // Calculate badge count for this user
    let notification_count = crate::db::social::get_unread_count(pool, user_id)
        .await
        .unwrap_or_else(|e| {
            error!("Failed to get unread notification count for user {}: {}", user_id, e);
            0
        });

    let message_count = crate::db::chat::get_unread_message_count(pool, user_id)
        .await
        .unwrap_or_else(|e| {
            error!("Failed to get unread message count for user {}: {}", user_id, e);
            0
        });

    let badge_count = (notification_count + message_count) as i32;

    // Build messages
    let mut messages: Vec<ExpoPushMessage> = Vec::new();

    for token in tokens {
        let mut message = ExpoPushMessage::new(
            token.token,
            req.title.clone(),
            req.body.clone(),
        );

        if let Some(ref data) = req.data {
            message = message.with_data(data.clone());
        }

        if let Some(ref notification_type) = req.notification_type {
            let channel = match notification_type.as_str() {
                "league_update" => "league_updates",
                "game_event" => "game_events",
                "health_reminder" => "health_reminders",
                "team_message" => "team_messages",
                _ => "default",
            };
            message = message.with_channel(channel.to_string());
        }

        message = message.with_badge(badge_count);

        messages.push(message);
    }

    // Send to Expo
    info!("📤 Sending {} push messages to Expo for user {}", messages.len(), user_id);
    let client = reqwest::Client::new();
    let response = client
        .post("https://exp.host/--/api/v2/push/send")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .json(&messages)
        .send()
        .await
        .map_err(|e| {
            error!("❌ HTTP error sending notification for user {}: {}", user_id, e);
            format!("HTTP error: {}", e)
        })?;

    let status = response.status();
    info!("📥 Expo response status for user {}: {}", user_id, status);

    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        error!("❌ Expo error for user {}: {}", user_id, error_text);
        return Err(format!("Expo error: {}", error_text));
    }

    // Parse response to check for ticket errors
    let response_text = response.text().await.unwrap_or_else(|_| "{}".to_string());
    info!("📋 Expo response for user {}: {}", user_id, response_text);

    if let Ok(expo_response) = serde_json::from_str::<serde_json::Value>(&response_text) {
        if let Some(data) = expo_response.get("data").and_then(|d| d.as_array()) {
            for (idx, ticket) in data.iter().enumerate() {
                if let Some(status) = ticket.get("status").and_then(|s| s.as_str()) {
                    if status == "error" {
                        let message = ticket.get("message").and_then(|m| m.as_str()).unwrap_or("unknown");
                        let details = ticket.get("details").and_then(|d| d.as_str()).unwrap_or("");
                        error!("❌ Expo ticket {} error for user {}: {} - {}", idx, user_id, message, details);
                    } else {
                        info!("✅ Expo ticket {} success for user {}", idx, user_id);
                    }
                }
            }
        }
    }

    info!("✅ Successfully sent notification to user {}", user_id);
    Ok(())
}

/// Get combined badge count (unread notifications + unread messages)
pub async fn get_badge_count(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> actix_web::Result<HttpResponse> {
    let user_id = Uuid::parse_str(&claims.sub).map_err(|e| {
        error!("Failed to parse user_id from claims: {}", e);
        actix_web::error::ErrorInternalServerError("Invalid user ID")
    })?;

    // Get unread notification count
    let notification_count = crate::db::social::get_unread_count(&pool, user_id)
        .await
        .map_err(|e| {
            error!("Failed to get unread notification count: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to get notification count")
        })?;

    // Get unread message count
    let message_count = crate::db::chat::get_unread_message_count(&pool, user_id)
        .await
        .map_err(|e| {
            error!("Failed to get unread message count: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to get message count")
        })?;

    let total_count = notification_count + message_count;

    info!("Badge count for user {}: {} notifications, {} messages, {} total",
          user_id, notification_count, message_count, total_count);

    Ok(HttpResponse::Ok().json(json!({
        "badge_count": total_count,
        "notification_count": notification_count,
        "message_count": message_count,
    })))
}
