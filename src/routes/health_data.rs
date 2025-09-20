use actix_web::{post, get, web, HttpResponse};
use crate::handlers::workout_data::upload_workout_data::upload_workout_data;
use crate::handlers::workout_data::media_upload::{request_upload_signed_url, confirm_upload, get_download_signed_url, UploadUrlRequest, ConfirmUploadRequest};
use crate::middleware::auth::Claims;
use crate::models::workout_data::WorkoutDataUploadRequest;
use crate::services::MinIOService;
use crate::config::jwt::JwtSettings;
use std::sync::Arc;

#[post("/upload_health")]
async fn upload_health(
    data: web::Json<WorkoutDataUploadRequest>,
    pool: web::Data<sqlx::PgPool>,
    redis: Option<web::Data<Arc<redis::Client>>>,
    claims: web::ReqData<Claims>,
    jwt_settings: web::Data<JwtSettings>,
) -> HttpResponse {
    upload_workout_data(data, pool, redis, claims, jwt_settings).await
}

#[post("/request-upload-url")]
async fn request_upload_url(
    request: web::Json<UploadUrlRequest>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>
) -> HttpResponse {
    request_upload_signed_url(request, claims, minio_service).await
}

#[post("/confirm-upload")]
async fn confirm_upload_handler(
    request: web::Json<ConfirmUploadRequest>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>,
    pool: web::Data<sqlx::PgPool>
) -> HttpResponse {
    confirm_upload(request, claims, minio_service, pool).await
}

#[get("/workout-media-url/{user_id}/{filename}")]
async fn get_download_url(
    path: web::Path<(String, String)>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>
) -> HttpResponse {
    get_download_signed_url(path, claims, minio_service).await
}