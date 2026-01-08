use actix_web::{web, HttpResponse};
use uuid::Uuid;
use sha2::{Sha256, Digest};

use crate::middleware::auth::Claims;
use crate::models::common::ApiResponse;
use crate::services::MinIOService;

// Response types for signed URL operations

#[derive(serde::Serialize)]
pub struct SignedUrlResponse {
    pub url: String,
    pub expires_in: u32,
    pub expected_hash: String,
}

#[derive(serde::Deserialize)]
pub struct UploadUrlRequest {
    pub filename: String,
    pub content_type: String,
    pub expected_hash: String,
    pub file_size: usize, // File size in bytes for validation
}

#[derive(serde::Serialize)]
pub struct UploadUrlResponse {
    pub upload_url: String,
    pub expires_in: u32,
    pub object_key: String,
}

#[derive(serde::Deserialize)]
pub struct ConfirmUploadRequest {
    pub object_key: String,
    pub expected_hash: String,
}

#[derive(serde::Serialize)]
pub struct ConfirmUploadResponse {
    pub success: bool,
    pub file_url: String,
    pub verified_hash: String,
}

// Constants
const ALLOWED_IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "heic", "heif"];
const ALLOWED_VIDEO_EXTENSIONS: &[&str] = &[
    "mp4",  // iOS/Android - most common
    "mov",  // iOS - QuickTime format
    "m4v",  // iOS - iTunes video
    "3gp",  // Android - older devices
    "webm", // Modern web/Android
];

// File size limits (in bytes)
const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024; // 10 MB
const MAX_VIDEO_SIZE: usize = 100 * 1024 * 1024; // 100 MB

// Helper to determine if extension is a video
fn is_video_extension(extension: &str) -> bool {
    ALLOWED_VIDEO_EXTENSIONS.contains(&extension)
}

// Helper to determine if extension is an image
fn is_image_extension(extension: &str) -> bool {
    ALLOWED_IMAGE_EXTENSIONS.contains(&extension)
}

// Request upload signed URL
pub async fn request_upload_signed_url(
    request: web::Json<UploadUrlRequest>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>,
) -> HttpResponse {
    let Some(user_id) = claims.user_id() else {
        tracing::error!("Invalid user ID in claims");
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid user ID"));
    };

    tracing::info!("📤 User {} requesting upload URL for: {}", claims.username, request.filename);

    // Validate file extension
    let extension = get_file_extension(&request.filename);
    if extension.is_empty() {
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error("File must have an extension")
        );
    }

    let is_video = is_video_extension(&extension);
    let is_image = is_image_extension(&extension);

    if !is_video && !is_image {
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error(&format!(
                "Invalid file type. Allowed images: {:?}, videos: {:?}",
                ALLOWED_IMAGE_EXTENSIONS,
                ALLOWED_VIDEO_EXTENSIONS
            ))
        );
    }

    // Validate file size based on type
    if is_image && request.file_size > MAX_IMAGE_SIZE {
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error(&format!(
                "Image file too large. Maximum size: {} MB",
                MAX_IMAGE_SIZE / (1024 * 1024)
            ))
        );
    }

    if is_video && request.file_size > MAX_VIDEO_SIZE {
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error(&format!(
                "Video file too large. Maximum size: {} MB",
                MAX_VIDEO_SIZE / (1024 * 1024)
            ))
        );
    }

    tracing::info!("📊 File validation passed: {} ({} bytes, is_video: {})",
                   request.filename, request.file_size, is_video);

    // Validate hash format (should be hex)
    if request.expected_hash.len() != 64 || !request.expected_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error("Invalid hash format - expected 64 character hex string")
        );
    }

    // Generate unique filename with proper extension
    let unique_filename = format!("{}.{}", Uuid::new_v4(), extension);
    let object_key = format!("media/{}/{}", user_id, unique_filename);

    // Generate signed upload URL with checksum verification
    match minio_service.generate_presigned_upload_url(&object_key, &request.content_type, &request.expected_hash, 3600).await {
        Ok(upload_url) => {
            tracing::info!("✅ Generated upload URL for {} (expires in 1h)", object_key);
            HttpResponse::Ok().json(ApiResponse::success(
                "Upload URL generated successfully",
                UploadUrlResponse {
                    upload_url,
                    expires_in: 3600,
                    object_key: object_key.clone(),
                }
            ))
        }
        Err(e) => {
            tracing::error!("❌ Failed to generate upload URL: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to generate upload URL")
            )
        }
    }
}

// Confirm upload completion and verify file
pub async fn confirm_upload(
    request: web::Json<ConfirmUploadRequest>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>,
) -> HttpResponse {
    tracing::info!("✅ User {} confirming upload: {}", claims.username, request.object_key);

    // Check if file exists and verify hash
    match minio_service.get_file(&request.object_key).await {
        Ok((contents, _content_type)) => {
            // Calculate actual file hash
            let mut hasher = Sha256::new();
            hasher.update(&contents);
            let actual_hash = format!("{:x}", hasher.finalize());

            // Verify hash matches expected
            if actual_hash != request.expected_hash {
                tracing::warn!("🚨 Hash mismatch after upload - Expected: {}, Actual: {}", 
                             request.expected_hash, actual_hash);
                return HttpResponse::BadRequest().json(
                    ApiResponse::<()>::error("Hash verification failed after upload")
                );
            }

            let file_url = minio_service.generate_file_url(&request.object_key);
            
            tracing::info!("✅ Upload confirmed and verified: {} (hash: {})", 
                          request.object_key, actual_hash);
            
            // Media upload confirmed - the file URL is returned to the client
            // The client can then use this URL to update posts or other entities as needed
            
            HttpResponse::Ok().json(ApiResponse::success(
                "Upload confirmed and verified successfully",
                ConfirmUploadResponse {
                    success: true,
                    file_url,
                    verified_hash: actual_hash,
                }
            ))
        }
        Err(e) => {
            tracing::warn!("❌ File not found after upload: {} - {}", request.object_key, e);
            HttpResponse::NotFound().json(
                ApiResponse::<()>::error("File not found - upload may have failed")
            )
        }
    }
}

// Get download signed URL for existing file
pub async fn get_download_signed_url(
    path: web::Path<(String, String)>, // (user_id, filename)
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>,
) -> HttpResponse {
    let (user_id_str, filename) = path.into_inner();
    tracing::info!("🔗 User {} requesting download URL for: {}/{}", 
                   claims.username, user_id_str, filename);
    
    // Validate filename to prevent directory traversal
    if filename.contains("..") || filename.contains("/") || filename.contains("\\") ||
       user_id_str.contains("..") || user_id_str.contains("/") || user_id_str.contains("\\") {
        tracing::warn!("🚨 Directory traversal attempt detected: {}/{}", user_id_str, filename);
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error("Invalid path parameters")
        );
    }

    // Validate that user_id_str is a valid UUID format
    if let Err(_) = uuid::Uuid::parse_str(&user_id_str) {
        tracing::warn!("Invalid user ID format: {}", user_id_str);
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error("Invalid user ID format")
        );
    }

    // All authenticated users can access all files (as requested)
    // Try new format first (media/), then fall back to legacy format (users/)
    let new_object_key = format!("media/{}/{}", user_id_str, filename);
    let legacy_object_key = format!("users/{}/{}", user_id_str, filename);

    // Try to get file from new location first, then legacy location
    let (object_key, get_result) = match minio_service.get_file(&new_object_key).await {
        Ok(result) => (new_object_key, Ok(result)),
        Err(_) => {
            tracing::debug!("🔍 File not found at new location, trying legacy location: {}", legacy_object_key);
            (legacy_object_key.clone(), minio_service.get_file(&legacy_object_key).await)
        }
    };

    // Check if file exists and get its hash
    match get_result {
        Ok((contents, _content_type)) => {
            // Calculate file hash for integrity verification
            let mut hasher = Sha256::new();
            hasher.update(&contents);
            let file_hash = format!("{:x}", hasher.finalize());
            
            // Generate signed download URL
            match minio_service.generate_presigned_download_url(&object_key, 3600).await {
                Ok(signed_url) => {
                    tracing::info!("✅ Generated download URL for {} (expires in 1h, hash: {})", 
                                  object_key, file_hash);
                    HttpResponse::Ok().json(ApiResponse::success(
                        "Download URL generated successfully",
                        SignedUrlResponse {
                            url: signed_url,
                            expires_in: 3600,
                            expected_hash: file_hash,
                        }
                    ))
                }
                Err(e) => {
                    tracing::error!("❌ Failed to generate download URL: {}", e);
                    HttpResponse::InternalServerError().json(
                        ApiResponse::<()>::error("Failed to generate download URL")
                    )
                }
            }
        }
        Err(e) => {
            tracing::warn!("❌ File not found: {} - {}", object_key, e);
            HttpResponse::NotFound().json(
                ApiResponse::<()>::error("File not found")
            )
        }
    }
}

// Helper function to extract file extension
fn get_file_extension(filename: &str) -> String {
    filename
        .split('.')
        .last()
        .unwrap_or("")
        .to_lowercase()
}