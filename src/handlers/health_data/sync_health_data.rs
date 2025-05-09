use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;
use crate::middleware::auth::Claims;
use crate::db::health_data::insert_health_data;
use crate::models::health_data::{HealthDataSyncRequest, HealthDataSyncResponse};

#[tracing::instrument(
    name = "Sync health data",
    skip(data, pool, claims),
    fields(
        username = %claims.username,
        data_type = %data.device_id
    )
)]

pub async fn sync_health_data(
    data: web::Json<HealthDataSyncRequest>,
    pool: web::Data<sqlx::PgPool>,
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