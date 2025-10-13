use actix_web::{web, get, put, post, HttpResponse};
use sqlx::PgPool;
use crate::handlers::profile::profile::get_user_profile;
use crate::handlers::profile::health_profile::{get_health_profile, update_health_profile};
use crate::handlers::profile::profile_picture::{
    request_profile_picture_upload_url, 
    confirm_profile_picture_upload, 
    get_profile_picture_download_url
};
use crate::middleware::auth::Claims;
use crate::models::profile::UpdateHealthProfileRequest;
use crate::services::MinIOService;

#[get("/user")]
async fn get_user(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    get_user_profile(pool, claims).await
}

#[get("/health_profile")]
async fn get_health_prof(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    get_health_profile(pool, claims).await
}

#[put("/health_profile")]
async fn update_health_prof(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    data: web::Json<UpdateHealthProfileRequest>,
) -> HttpResponse {
    update_health_profile(pool, claims, data).await
}

// Profile picture upload routes
#[post("/picture/request-upload-url")]
async fn request_profile_picture_upload_url_handler(
    request: web::Json<crate::handlers::profile::profile_picture::ProfilePictureUploadUrlRequest>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>
) -> HttpResponse {
    request_profile_picture_upload_url(request, claims, minio_service).await
}

#[post("/picture/confirm-upload")]
async fn confirm_profile_picture_upload_handler(
    request: web::Json<crate::handlers::profile::profile_picture::ConfirmProfilePictureUploadRequest>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>,
    pool: web::Data<PgPool>
) -> HttpResponse {
    confirm_profile_picture_upload(request, claims, minio_service, pool).await
}

#[get("/picture/download-url/{user_id}")]
async fn get_profile_picture_download_url_handler(
    path: web::Path<uuid::Uuid>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>
) -> HttpResponse {
    get_profile_picture_download_url(path, claims, minio_service).await
}