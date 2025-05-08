use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use sqlx::{Pool, Postgres};

use crate::auth::jwt::AuthenticatedUser;
use crate::db::health_data::insert_health_data;
use crate::models::health_data::{HealthDataSyncRequest, HealthDataSyncResponse};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/health-data")
            .route("", web::post().to(sync_health_data)),
    );
}

async fn sync_health_data(
    pool: web::Data<Pool<Postgres>>,
    authenticated_user: AuthenticatedUser,
    data: web::Json<HealthDataSyncRequest>,
) -> impl Responder {
    // Insert health data into database
    let insert_result = insert_health_data(&pool, authenticated_user.user_id, &data).await;
    
    match insert_result {
        Ok(sync_id) => {
            // Prepare successful response
            let response = HealthDataSyncResponse {
                success: true,
                message: "Health data synced successfully".to_string(),
                sync_id,
                timestamp: Utc::now(),
            };
            
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            // Log the error
            log::error!("Failed to sync health data: {}", e);
            
            // Prepare error response
            let response = HealthDataSyncResponse {
                success: false,
                message: format!("Failed to sync health data: {}", e),
                sync_id: uuid::Uuid::nil(), // Use nil UUID for error case
                timestamp: Utc::now(),
            };
            
            HttpResponse::InternalServerError().json(response)
        }
    }
}