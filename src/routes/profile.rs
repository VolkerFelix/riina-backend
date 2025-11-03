use actix_web::{web, get, put, post, patch, HttpResponse};
use sqlx::PgPool;
use crate::handlers::profile::profile::{get_user_profile, UserProfileQuery};
use crate::handlers::profile::health_profile::{get_health_profile, update_health_profile, HealthProfileQuery};
use crate::handlers::profile::profile_picture::{
    request_profile_picture_upload_url,
    confirm_profile_picture_upload,
    get_profile_picture_download_url
};
use crate::handlers::profile::user_status::{update_user_status, get_user_status};
use crate::middleware::auth::Claims;
use crate::models::profile::UpdateHealthProfileRequest;
use crate::models::user::UpdateUserStatusRequest;
use crate::services::MinIOService;

#[get("/user")]
async fn get_user(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    query: web::Query<UserProfileQuery>
) -> HttpResponse {
    get_user_profile(pool, claims, query).await
}

#[get("/health_profile")]
async fn get_health_prof(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    query: web::Query<HealthProfileQuery>
) -> HttpResponse {
    get_health_profile(pool, claims, query).await
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
    minio_service: web::Data<MinIOService>,
    pool: web::Data<PgPool>
) -> HttpResponse {
    get_profile_picture_download_url(path, claims, minio_service, pool).await
}

// Serve profile picture files (requires authentication)
#[get("/picture/{user_id}/{filename}")]
async fn serve_profile_picture(
    path: web::Path<(String, String)>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>
) -> HttpResponse {
    let (user_id, filename) = path.into_inner();

    // Authorization: authenticated users can view any profile picture
    // This allows users to see each other's profile pictures in the app
    tracing::info!("📸 User {} requesting profile picture for user {}", claims.sub, user_id);

    let object_key = format!("profile-pictures/{}/{}", user_id, filename);

    match minio_service.get_file(&object_key).await {
        Ok((contents, content_type)) => {
            tracing::info!("✅ Serving profile picture for user {}: {}", user_id, filename);
            HttpResponse::Ok()
                .content_type(content_type)
                .body(contents)
        }
        Err(e) => {
            tracing::warn!("❌ Profile picture not found for user {}: {} - {}", user_id, filename, e);
            HttpResponse::NotFound().finish()
        }
    }
}

// User status routes
#[get("/status")]
async fn get_status(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> HttpResponse {
    get_user_status(pool, claims).await
}

#[patch("/status")]
async fn update_status(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    request: web::Json<UpdateUserStatusRequest>,
) -> HttpResponse {
    update_user_status(pool, claims, request).await
}