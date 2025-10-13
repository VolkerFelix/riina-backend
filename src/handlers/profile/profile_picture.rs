use actix_web::{web, HttpResponse};
use uuid::Uuid;
use sha2::{Sha256, Digest};

use crate::middleware::auth::Claims;
use crate::models::common::ApiResponse;
use crate::services::MinIOService;

// Response types for profile picture upload operations

#[derive(serde::Serialize)]
pub struct ProfilePictureUploadUrlResponse {
    pub upload_url: String,
    pub expires_in: u32,
    pub object_key: String,
}

#[derive(serde::Deserialize)]
pub struct ProfilePictureUploadUrlRequest {
    pub filename: String,
    pub content_type: String,
    pub expected_hash: String,
}

#[derive(serde::Deserialize)]
pub struct ConfirmProfilePictureUploadRequest {
    pub object_key: String,
    pub expected_hash: String,
}

#[derive(serde::Serialize)]
pub struct ConfirmProfilePictureUploadResponse {
    pub success: bool,
    pub file_url: String,
    pub verified_hash: String,
}

// Constants for profile picture uploads
const ALLOWED_PROFILE_PICTURE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif"];
const MAX_PROFILE_PICTURE_SIZE: usize = 5 * 1024 * 1024; // 5MB

// Request upload signed URL for profile picture
pub async fn request_profile_picture_upload_url(
    request: web::Json<ProfilePictureUploadUrlRequest>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>,
) -> HttpResponse {
    tracing::info!("📸 User {} requesting profile picture upload URL for: {}", 
                   claims.username, request.filename);

    // Validate file extension
    let extension = get_file_extension(&request.filename).to_lowercase();
    if !ALLOWED_PROFILE_PICTURE_EXTENSIONS.contains(&extension.as_str()) {
        tracing::warn!("🚫 Invalid file extension for profile picture: {}", extension);
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error("Invalid file type. Only JPG, JPEG, PNG, and GIF are allowed for profile pictures")
        );
    }

    // Parse user ID from claims
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            tracing::error!("Invalid user ID in claims");
            return HttpResponse::BadRequest().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };

    // Generate unique object key for profile picture
    let timestamp = chrono::Utc::now().timestamp();
    let object_key = format!("profile-pictures/{}/{}_{}", user_id, timestamp, request.filename);

    // Generate signed URL for upload
    match minio_service.generate_presigned_upload_url(&object_key, &request.content_type, &request.expected_hash, 3600).await {
        Ok(upload_url) => {
            tracing::info!("✅ Generated profile picture upload URL for user {}: {}", 
                          user_id, object_key);
            
            HttpResponse::Ok().json(ApiResponse::success(
                "Upload URL generated successfully",
                ProfilePictureUploadUrlResponse {
                    upload_url,
                    expires_in: 3600,
                    object_key,
                }
            ))
        }
        Err(e) => {
            tracing::error!("❌ Failed to generate profile picture upload URL: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to generate upload URL")
            )
        }
    }
}

// Confirm profile picture upload and update user record
pub async fn confirm_profile_picture_upload(
    request: web::Json<ConfirmProfilePictureUploadRequest>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>,
    pool: web::Data<sqlx::PgPool>,
) -> HttpResponse {
    tracing::info!("✅ User {} confirming profile picture upload: {}", 
                   claims.username, request.object_key);

    // Parse user ID from claims
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            tracing::error!("Invalid user ID in claims");
            return HttpResponse::BadRequest().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };

    // Check if file exists and verify hash
    match minio_service.get_file(&request.object_key).await {
        Ok((contents, _content_type)) => {
            // Verify file size
            if contents.len() > MAX_PROFILE_PICTURE_SIZE {
                tracing::warn!("🚫 Profile picture too large: {} bytes (max: {} bytes)", 
                             contents.len(), MAX_PROFILE_PICTURE_SIZE);
                return HttpResponse::BadRequest().json(
                    ApiResponse::<()>::error("Profile picture too large. Maximum size is 5MB")
                );
            }

            // Calculate actual file hash
            let mut hasher = Sha256::new();
            hasher.update(&contents);
            let actual_hash = format!("{:x}", hasher.finalize());

            // Verify hash matches expected
            if actual_hash != request.expected_hash {
                tracing::warn!("🚨 Hash mismatch after profile picture upload - Expected: {}, Actual: {}", 
                             request.expected_hash, actual_hash);
                return HttpResponse::BadRequest().json(
                    ApiResponse::<()>::error("Hash verification failed after upload")
                );
            }

            let file_url = minio_service.generate_file_url(&request.object_key);
            
            tracing::info!("✅ Profile picture upload confirmed and verified: {} (hash: {})", 
                          request.object_key, actual_hash);
            
            // Update user record with profile picture URL
            match sqlx::query!(
                "UPDATE users SET profile_picture_url = $1, updated_at = NOW() WHERE id = $2",
                file_url,
                user_id
            )
            .execute(pool.get_ref())
            .await
            {
                Ok(_) => {
                    tracing::info!("✅ Updated user {} with profile picture URL: {}", user_id, file_url);
                    HttpResponse::Ok().json(ApiResponse {
                        data: ConfirmProfilePictureUploadResponse {
                            success: true,
                            file_url,
                            verified_hash: actual_hash,
                        },
                        success: true,
                        message: Some("Profile picture uploaded successfully".to_string()),
                    })
                }
                Err(e) => {
                    tracing::error!("❌ Failed to update user profile picture: {}", e);
                    HttpResponse::InternalServerError().json(
                        ApiResponse::<()>::error("Failed to update profile picture")
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to verify profile picture upload: {}", e);
            HttpResponse::BadRequest().json(
                ApiResponse::<()>::error("Failed to verify uploaded file")
            )
        }
    }
}

// Get profile picture download URL
pub async fn get_profile_picture_download_url(
    path: web::Path<Uuid>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>,
) -> HttpResponse {
    let user_id = path.into_inner();
    
    tracing::info!("📸 User {} requesting profile picture download URL for user: {}", 
                   claims.username, user_id);

    // Generate download URL for the profile picture
    let object_key = format!("profile-pictures/{}/", user_id);
    
    match minio_service.generate_download_signed_url(&object_key, 3600).await {
        Ok((download_url, expires_in)) => {
            tracing::info!("✅ Generated profile picture download URL for user {}", user_id);
            HttpResponse::Ok().json(ApiResponse {
                data: serde_json::json!({
                    "download_url": download_url,
                    "expires_in": expires_in
                }),
                success: true,
                message: Some("Download URL generated successfully".to_string()),
            })
        }
        Err(e) => {
            tracing::error!("❌ Failed to generate profile picture download URL: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to generate download URL")
            )
        }
    }
}

// Helper function to extract file extension
fn get_file_extension(filename: &str) -> &str {
    filename
        .rfind('.')
        .map(|pos| &filename[pos + 1..])
        .unwrap_or("")
}
