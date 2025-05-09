use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;
use crate::middleware::auth::Claims;
use crate::db::health_data::insert_health_data;
use crate::models::health_data::{HealthDataSyncRequest, HealthDataSyncResponse};
use redis::AsyncCommands;

#[tracing::instrument(
    name = "Sync health data",
    skip(data, pool, claims),
    fields(
        username = %claims.username,
        data_type = %data.device_id
    )
)]

pub async fn upload_health_data(
    data: web::Json<HealthDataSyncRequest>,
    pool: web::Data<sqlx::PgPool>,
    redis: web::Data<redis::Client>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    tracing::info!("Sync health data handler called from device: {}", data.device_id);
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => {
            tracing::info!("User ID parsed successfully: {}", id);
            id
        },
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": "Invalid user ID"
            }));
        }
    };    
    // Insert health data into database
    let insert_result = insert_health_data(&pool, user_id, &data).await;
    
    match insert_result {
        Ok(sync_id) => {
            // Publish event to Redis
            match redis.get_async_connection().await {
                Ok(mut conn) => {
                    let event = serde_json::json!({
                        "event_type": "health_data_uploaded",
                        "user_id": user_id.to_string(),
                        "sync_id": sync_id.to_string(),
                        "timestamp": Utc::now().to_rfc3339()
                    });
                    // Publish event to Redis channel
                    match conn.publish::<_, String, String>("evolveme:events:health_data", event.to_string()).await {
                        Ok(_) => tracing::info!("Successfully published health data event for sync_id: {}", sync_id),
                        Err(e) => tracing::error!("Failed to publish health data event: {}", e),
                    }
                },
                Err(e) => tracing::error!("Failed to connect to Redis: {}", e),
            }
            // Prepare successful response
            let response = HealthDataSyncResponse {
                success: true,
                message: "Health data synced successfully".to_string(),
                sync_id,
                timestamp: Utc::now(),
            };
            tracing::info!("Health data synced successfully: {}", sync_id); 
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            // Prepare error response
            let response = HealthDataSyncResponse {
                success: false,
                message: format!("Failed to sync health data: {}", e),
                sync_id: uuid::Uuid::nil(), // Use nil UUID for error case
                timestamp: Utc::now(),
            };
            tracing::error!("Failed to sync health data: {}", e);
            HttpResponse::InternalServerError().json(response)
        }
    }
}