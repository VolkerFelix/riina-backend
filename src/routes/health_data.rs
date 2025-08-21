use actix_web::{post, get, put, web, HttpResponse};
use crate::handlers::workout_data::upload_workout_data::upload_workout_data;
use crate::handlers::workout_data::media_upload::{request_upload_signed_url, confirm_upload, get_download_signed_url};
use crate::handlers::workout_data::update_workout_media::update_workout_media;
use crate::middleware::auth::Claims;
use crate::models::workout_data::WorkoutDataSyncRequest;
use crate::services::{live_game_service::LiveGameService, MinIOService};
use std::sync::Arc;

#[post("/upload_health")]
async fn upload_health(
    data: web::Json<WorkoutDataSyncRequest>,
    pool: web::Data<sqlx::PgPool>,
    redis: Option<web::Data<Arc<redis::Client>>>,
    live_game_service: Option<web::Data<LiveGameService>>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    upload_workout_data(data, pool, redis, live_game_service, claims).await
}

// Old upload_media and serve_media endpoints removed - now using signed URLs

#[put("/workout/{workout_id}/media")]
async fn update_media(
    workout_id: web::Path<String>,
    data: web::Json<crate::handlers::workout_data::update_workout_media::UpdateWorkoutMediaRequest>,
    pool: web::Data<sqlx::PgPool>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    // Parse workout_id from path and merge with request data
    let workout_uuid = match uuid::Uuid::parse_str(&workout_id.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(
                crate::models::common::ApiResponse::<()>::error("Invalid workout ID")
            );
        }
    };
    
    let mut request_data = data.into_inner();
    request_data.workout_id = workout_uuid;
    
    update_workout_media(web::Json(request_data), pool, claims).await
}

#[post("/request-upload-url")]
async fn request_upload_url(
    request: web::Json<crate::handlers::workout_data::media_upload::UploadUrlRequest>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>
) -> HttpResponse {
    request_upload_signed_url(request, claims, minio_service).await
}

#[post("/confirm-upload")]
async fn confirm_upload_handler(
    request: web::Json<crate::handlers::workout_data::media_upload::ConfirmUploadRequest>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>
) -> HttpResponse {
    confirm_upload(request, claims, minio_service).await
}

#[get("/workout-media-url/{user_id}/{filename}")]
async fn get_download_url(
    path: web::Path<(String, String)>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>
) -> HttpResponse {
    get_download_signed_url(path, claims, minio_service).await
}