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
    pub workout_id: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ConfirmUploadResponse {
    pub success: bool,
    pub file_url: String,
    pub verified_hash: String,
}

// Constants
const ALLOWED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "mp4", "mov", "avi"];

// Request upload signed URL
pub async fn request_upload_signed_url(
    request: web::Json<UploadUrlRequest>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>,
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };

    tracing::info!("📤 User {} requesting upload URL for: {}", claims.username, request.filename);

    // Validate file extension
    let extension = get_file_extension(&request.filename);
    if extension.is_empty() || !ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error("Invalid file type")
        );
    }

    // Validate hash format (should be hex)
    if request.expected_hash.len() != 64 || !request.expected_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error("Invalid hash format - expected 64 character hex string")
        );
    }

    // Generate unique filename with proper extension
    let unique_filename = format!("{}.{}", Uuid::new_v4(), extension);
    let object_key = format!("users/{}/{}", user_id, unique_filename);

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
    pool: web::Data<sqlx::PgPool>,
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
            
            // If workout_id is provided, update the workout record with the media URL
            if let Some(workout_id_str) = &request.workout_id {
                if let Ok(workout_uuid) = uuid::Uuid::parse_str(workout_id_str) {
                    let user_id = match uuid::Uuid::parse_str(&claims.sub) {
                        Ok(id) => id,
                        Err(_) => {
                            tracing::error!("Invalid user ID in claims");
                            return HttpResponse::InternalServerError().json(
                                ApiResponse::<()>::error("Invalid user ID")
                            );
                        }
                    };
                    
                    // Determine if this is an image or video based on file extension
                    let extension = get_file_extension(&request.object_key).to_lowercase();
                    let is_image = matches!(extension.as_str(), "jpg" | "jpeg" | "png" | "gif");
                    
                    // Update the workout_data table
                    let update_query = if is_image {
                        "UPDATE workout_data SET image_url = $1 WHERE id = $2 AND user_id = $3"
                    } else {
                        "UPDATE workout_data SET video_url = $1 WHERE id = $2 AND user_id = $3"
                    };
                    
                    match sqlx::query(update_query)
                        .bind(&file_url)
                        .bind(workout_uuid)
                        .bind(user_id)
                        .execute(pool.get_ref())
                        .await
                    {
                        Ok(result) => {
                            if result.rows_affected() > 0 {
                                tracing::info!("✅ Updated workout {} with {} URL: {}", 
                                             workout_uuid, if is_image { "image" } else { "video" }, file_url);
                            } else {
                                tracing::warn!("⚠️ No workout found with ID {} for user {}", workout_uuid, user_id);
                            }
                        }
                        Err(e) => {
                            tracing::error!("❌ Failed to update workout with media URL: {}", e);
                            // Continue anyway, don't fail the upload
                        }
                    }
                } else {
                    tracing::warn!("Invalid workout_id format: {}", workout_id_str);
                }
            }
            
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
    let object_key = format!("users/{}/{}", user_id_str, filename);
    
    // Check if file exists and get its hash
    match minio_service.get_file(&object_key).await {
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