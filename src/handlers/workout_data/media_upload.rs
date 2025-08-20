use actix_multipart::{form::tempfile::TempFile, form::MultipartForm};
use actix_web::{web, HttpResponse};
use std::fs;
use uuid::Uuid;
use sha2::{Sha256, Digest};
use std::io::Read;
use bytes::Bytes;

use crate::middleware::auth::Claims;
use crate::models::common::ApiResponse;
use crate::services::MinIOService;

#[derive(Debug, MultipartForm)]
pub struct MediaUploadForm {
    #[multipart(limit = "50MB")]
    pub file: TempFile,
    pub workout_id: Option<actix_multipart::form::text::Text<String>>,
}

#[derive(serde::Serialize)]
pub struct MediaUploadResponse {
    pub file_url: String,
    pub file_type: String,
    pub file_size: u64,
    pub file_hash: String,
}

const UPLOAD_DIR: &str = "./uploads/workout_media";
const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50MB
const ALLOWED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "mp4", "mov", "avi"];

#[tracing::instrument(
    name = "Upload workout media",
    skip(form, claims, minio_service),
    fields(
        username = %claims.username,
        file_name = "deferred"
    )
)]
pub async fn upload_workout_media(
    form: MultipartForm<MediaUploadForm>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>,
) -> HttpResponse {
    tracing::info!("📎 Processing media upload for user: {}", claims.username);

    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };

    let form_data = form.into_inner();
    
    // Log file info for debugging
    tracing::info!("📎 File upload details - Name: {:?}, Size: {:?}, Content-Type: {:?}", 
        form_data.file.file_name, 
        form_data.file.size, 
        form_data.file.content_type
    );

    // Validate file
    if let Err(error_response) = validate_file(&form_data.file) {
        return error_response;
    }

    // Generate unique filename with proper extension
    let original_filename = form_data.file.file_name.as_deref().unwrap_or("unknown");
    let mut extension = get_file_extension(original_filename);
    
    // If no extension or invalid extension, try to determine from content type
    if extension.is_empty() || !ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
        if let Some(content_type) = &form_data.file.content_type {
            let mime_str = content_type.to_string();
            extension = match mime_str.as_str() {
                "image/jpeg" | "image/jpg" => "jpg".to_string(),
                "image/png" => "png".to_string(),
                "image/gif" => "gif".to_string(),
                "video/mp4" => "mp4".to_string(),
                "video/quicktime" => "mov".to_string(),
                "video/avi" | "video/x-msvideo" => "avi".to_string(),
                _ => extension, // Keep original if can't determine
            };
        }
    }
    
    // Final fallback
    if extension.is_empty() {
        extension = "png".to_string(); // Default to png for images
    }
    
    let unique_filename = format!("{}.{}", Uuid::new_v4(), extension);

    // Read file data into memory
    let file_data = match std::fs::read(&form_data.file.file.path()) {
        Ok(data) => Bytes::from(data),
        Err(e) => {
            tracing::error!("Failed to read uploaded file: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to read uploaded file")
            );
        }
    };

    // Calculate file hash for integrity checking
    let file_hash = match calculate_file_hash(&form_data.file.file.path()).await {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!("Failed to calculate file hash: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to process file")
            );
        }
    };

    // Determine content type
    let content_type = form_data.file.content_type
        .as_ref()
        .map(|ct| ct.to_string())
        .unwrap_or_else(|| match extension.as_str() {
            "jpg" | "jpeg" => "image/jpeg".to_string(),
            "png" => "image/png".to_string(),
            "gif" => "image/gif".to_string(),
            "mp4" => "video/mp4".to_string(),
            "mov" => "video/quicktime".to_string(),
            "avi" => "video/avi".to_string(),
            _ => "application/octet-stream".to_string(),
        });

    // Upload to MinIO
    match minio_service.upload_file(file_data.clone(), &unique_filename, &content_type, user_id).await {
        Ok(object_key) => {
            let file_size = file_data.len() as u64;
            let file_url = minio_service.generate_file_url(&object_key);
            let file_type = determine_file_type(&extension);

            let response = MediaUploadResponse {
                file_url,
                file_type,
                file_size,
                file_hash,
            };

            tracing::info!("📎 Media upload completed for user {}: {}", claims.username, unique_filename);
            HttpResponse::Ok().json(
                ApiResponse::success("Media uploaded successfully", response)
            )
        }
        Err(e) => {
            tracing::error!("Failed to upload file to MinIO: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to upload file")
            )
        }
    }
}

fn validate_file(temp_file: &TempFile) -> Result<(), HttpResponse> {
    // Check file size
    let file_size = match fs::metadata(&temp_file.file.path()) {
        Ok(metadata) => metadata.len(),
        Err(_) => {
            return Err(HttpResponse::BadRequest().json(
                ApiResponse::<()>::error("Unable to read file metadata")
            ));
        }
    };

    if file_size > MAX_FILE_SIZE {
        return Err(HttpResponse::BadRequest().json(
            ApiResponse::<()>::error(&format!("File size exceeds maximum limit of {}MB", MAX_FILE_SIZE / 1024 / 1024))
        ));
    }

    if file_size == 0 {
        return Err(HttpResponse::BadRequest().json(
            ApiResponse::<()>::error("File is empty")
        ));
    }

    // Check file extension from filename, or fall back to content type
    let mut is_valid = false;
    
    if let Some(filename) = &temp_file.file_name {
        let extension = get_file_extension(filename);
        if ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
            is_valid = true;
        } else {
            tracing::warn!("File extension '{}' not in allowed list, checking content-type", extension);
        }
    }
    
    // If filename validation failed or no filename, check content type
    if !is_valid {
        if let Some(content_type) = &temp_file.content_type {
            let mime_str = content_type.to_string();
            tracing::info!("Checking content-type: {}", mime_str);
            
            // Map common mime types to extensions
            let allowed_mimes = vec![
                "image/jpeg", "image/jpg", "image/png", "image/gif",
                "video/mp4", "video/quicktime", "video/avi", "video/x-msvideo"
            ];
            
            if allowed_mimes.contains(&mime_str.as_str()) {
                is_valid = true;
                tracing::info!("Valid content-type found: {}", mime_str);
            }
        }
    }
    
    if !is_valid {
        return Err(HttpResponse::BadRequest().json(
            ApiResponse::<()>::error(&format!("File type not allowed. Supported types: {}", ALLOWED_EXTENSIONS.join(", ")))
        ));
    }

    Ok(())
}

fn get_file_extension(filename: &str) -> String {
    filename
        .split('.')
        .last()
        .unwrap_or("")
        .to_lowercase()
}

fn determine_file_type(extension: &str) -> String {
    match extension {
        "jpg" | "jpeg" | "png" | "gif" => "image".to_string(),
        "mp4" | "mov" | "avi" => "video".to_string(),
        _ => "unknown".to_string(),
    }
}

async fn calculate_file_hash(file_path: &std::path::Path) -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(file_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

// Handler to serve uploaded media files
#[tracing::instrument(
    name = "Serve workout media",
    skip(path, claims, minio_service),
    fields(
        username = %claims.username,
        user_id = "deferred",
        filename = "deferred"
    )
)]
pub async fn serve_workout_media(
    path: web::Path<(String, String)>,
    claims: web::ReqData<Claims>,
    minio_service: web::Data<MinIOService>,
) -> HttpResponse {
    let (user_id_str, filename) = path.into_inner();
    tracing::info!("🖼️ [SERVE MEDIA DEBUG] Authenticated user {} requesting file: {}/{}", claims.username, user_id_str, filename);
    
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

    // Reconstruct the object key
    let object_key = format!("users/{}/{}", user_id_str, filename);
    
    tracing::info!("🖼️ [SERVE MEDIA DEBUG] Looking for MinIO object: {}", object_key);
    
    match minio_service.get_file(&object_key).await {
        Ok((contents, content_type)) => {
            tracing::info!("✅ Successfully served file from MinIO: {} (size: {} bytes)", 
                          object_key, contents.len());
            HttpResponse::Ok()
                .content_type(content_type)
                .body(contents)
        }
        Err(e) => {
            tracing::warn!("❌ File not found in MinIO: {} - {}", object_key, e);
            HttpResponse::NotFound().json(
                ApiResponse::<()>::error("File not found")
            )
        }
    }
}