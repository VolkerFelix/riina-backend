//! Media upload test helpers
//!
//! Common functions for testing media uploads (images, videos, profile pictures)
//! to reduce duplication across test files

use reqwest::Client;
use serde_json::json;
use sha2::{Sha256, Digest};

pub enum MediaType {
    Image,
    Video,
    ProfilePicture,
}

pub struct MediaUploadResult {
    pub file_url: String,
    pub object_key: String,
}

/// Complete media upload workflow (request URL, upload, confirm)
///
/// This helper performs the full 3-step upload process:
/// 1. Request signed upload URL from backend
/// 2. Upload file to MinIO using signed URL
/// 3. Confirm upload completion with backend
pub async fn upload_test_media_file(
    client: &Client,
    app_address: &str,
    token: &str,
    filename: &str,
    content_type: &str,
    content: &[u8],
) -> Result<MediaUploadResult, String> {
    // Calculate hash of content
    let mut hasher = Sha256::new();
    hasher.update(content);
    let content_hash = format!("{:x}", hasher.finalize());

    // Step 1: Request upload URL
    let upload_response = client
        .post(&format!("{}/media/upload-url", app_address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "filename": filename,
            "content_type": content_type,
            "expected_hash": content_hash,
            "file_size": content.len()
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to request upload URL: {}", e))?;

    if !upload_response.status().is_success() {
        return Err(format!("Upload URL request failed with status: {}", upload_response.status()));
    }

    let upload_data: serde_json::Value = upload_response
        .json()
        .await
        .map_err(|e| format!("Failed to parse upload URL response: {}", e))?;

    let upload_url = upload_data["data"]["upload_url"]
        .as_str()
        .ok_or("Missing upload_url in response")?;
    let object_key = upload_data["data"]["object_key"]
        .as_str()
        .ok_or("Missing object_key in response")?;

    // Step 2: Upload to MinIO
    use base64::{Engine as _, engine::general_purpose};
    let hash_bytes = hex::decode(&content_hash)
        .map_err(|e| format!("Failed to decode hash: {}", e))?;
    let base64_hash = general_purpose::STANDARD.encode(&hash_bytes);

    let minio_response = client
        .put(upload_url)
        .header("Content-Type", content_type)
        .header("x-amz-checksum-sha256", base64_hash)
        .body(content.to_vec())
        .send()
        .await
        .map_err(|e| format!("Failed to upload to MinIO: {}", e))?;

    if !minio_response.status().is_success() {
        return Err(format!("MinIO upload failed with status: {}", minio_response.status()));
    }

    // Step 3: Confirm upload
    let confirm_response = client
        .post(&format!("{}/media/confirm-upload", app_address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "object_key": object_key,
            "expected_hash": content_hash
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to confirm upload: {}", e))?;

    if !confirm_response.status().is_success() {
        return Err(format!("Upload confirmation failed with status: {}", confirm_response.status()));
    }

    let confirm_data: serde_json::Value = confirm_response
        .json()
        .await
        .map_err(|e| format!("Failed to parse confirm response: {}", e))?;

    let file_url = confirm_data["data"]["file_url"]
        .as_str()
        .ok_or("Missing file_url in confirm response")?
        .to_string();

    Ok(MediaUploadResult {
        file_url,
        object_key: object_key.to_string(),
    })
}

/// Create test media content with specified size
pub fn create_test_media_content(media_type: MediaType, size: usize) -> Vec<u8> {
    let prefix: &[u8] = match media_type {
        MediaType::Image => b"fake image content: ",
        MediaType::Video => b"fake video content: ",
        MediaType::ProfilePicture => b"fake profile picture: ",
    };

    let mut content = prefix.to_vec();

    // Fill remaining bytes with pattern
    while content.len() < size {
        content.push(b'x');
    }

    content.truncate(size);
    content
}

/// Get the appropriate content type for media type
pub fn get_content_type(media_type: &MediaType) -> &'static str {
    match media_type {
        MediaType::Image | MediaType::ProfilePicture => "image/jpeg",
        MediaType::Video => "video/mp4",
    }
}

/// Get the appropriate filename for media type
pub fn get_test_filename(media_type: &MediaType) -> &'static str {
    match media_type {
        MediaType::Image => "test-image.jpg",
        MediaType::Video => "test-video.mp4",
        MediaType::ProfilePicture => "profile-picture.jpg",
    }
}
