//! Consolidated Media Upload Tests
//!
//! This file contains general media upload tests including:
//! - General media uploads (images, videos)
//! - Validation tests (format, size limits)
//! - Signed URL generation
//!
//! Replaces:
//! - media_test.rs (6 tests consolidated into 7 more focused tests)
//!
//! Note: Post video integration tests remain in post_videos_test.rs since they
//! test post creation/updates with video URLs.
//! Profile picture uploads remain in profile_picture_test.rs since they
//! use different endpoints (/profile/picture/*)

use reqwest::Client;
use serde_json::json;
use sha2::{Sha256, Digest};

mod common;
use common::utils::{spawn_app, create_test_user_and_login};
use common::media_helpers::{
    upload_test_media_file, create_test_media_content, MediaType
};

// ============================================================================
// IMAGE UPLOAD TESTS
// ============================================================================

#[tokio::test]
async fn test_image_upload_workflow() {
    let app = spawn_app().await;
    let client = Client::new();
    let test_user = create_test_user_and_login(&app.address).await;

    println!("📸 Testing image upload workflow");

    // Create test image content
    let test_content = create_test_media_content(MediaType::Image, 1024);

    // Upload using helper
    let result = upload_test_media_file(
        &client,
        &app.address,
        &test_user.token,
        "test-image.jpg",
        "image/jpeg",
        &test_content,
    )
    .await;

    assert!(result.is_ok(), "Image upload should succeed");
    let upload_result = result.unwrap();

    assert!(!upload_result.file_url.is_empty(), "Should have file URL");
    assert!(!upload_result.object_key.is_empty(), "Should have object key");

    println!("✅ Image upload workflow completed");
}

#[tokio::test]
async fn test_image_format_validation() {
    let app = spawn_app().await;
    let client = Client::new();
    let test_user = create_test_user_and_login(&app.address).await;

    println!("📸 Testing image format validation");

    let test_content = b"fake image data";
    let mut hasher = Sha256::new();
    hasher.update(test_content);
    let test_hash = format!("{:x}", hasher.finalize());

    // Test allowed formats
    let allowed_formats = vec!["jpg", "jpeg", "png", "gif", "heic", "heif"];

    for format in allowed_formats {
        let response = client
            .post(&format!("{}/media/upload-url", &app.address))
            .header("Authorization", format!("Bearer {}", test_user.token))
            .json(&json!({
                "filename": format!("test.{}", format),
                "content_type": format!("image/{}", format),
                "expected_hash": test_hash,
                "file_size": test_content.len()
            }))
            .send()
            .await
            .expect("Failed to request upload URL");

        assert_eq!(response.status(), 200, "Should accept .{} format", format);
    }

    // Test invalid format
    let response = client
        .post(&format!("{}/media/upload-url", &app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&json!({
            "filename": "test.bmp",
            "content_type": "image/bmp",
            "expected_hash": test_hash,
            "file_size": test_content.len()
        }))
        .send()
        .await
        .expect("Failed to request upload URL");

    assert_eq!(response.status(), 400, "Should reject .bmp format");

    println!("✅ Image format validation passed");
}

#[tokio::test]
async fn test_image_size_validation() {
    let app = spawn_app().await;
    let client = Client::new();
    let test_user = create_test_user_and_login(&app.address).await;

    println!("📸 Testing image size validation");

    let mut hasher = Sha256::new();
    hasher.update(b"test");
    let test_hash = format!("{:x}", hasher.finalize());

    // Try to upload image larger than 10MB limit
    let large_size = 11 * 1024 * 1024; // 11 MB

    let response = client
        .post(&format!("{}/media/upload-url", &app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&json!({
            "filename": "large-image.jpg",
            "content_type": "image/jpeg",
            "expected_hash": test_hash,
            "file_size": large_size
        }))
        .send()
        .await
        .expect("Failed to request upload URL");

    assert_eq!(response.status(), 400, "Should reject image over 10MB");

    let error_data: serde_json::Value = response.json().await.expect("Failed to parse error");
    assert!(error_data["error"].as_str().unwrap().contains("too large"));

    println!("✅ Image size validation passed");
}

// ============================================================================
// VIDEO UPLOAD TESTS
// ============================================================================

#[tokio::test]
async fn test_video_upload_workflow() {
    let app = spawn_app().await;
    let client = Client::new();
    let test_user = create_test_user_and_login(&app.address).await;

    println!("🎥 Testing video upload workflow");

    let test_content = create_test_media_content(MediaType::Video, 2048);

    let result = upload_test_media_file(
        &client,
        &app.address,
        &test_user.token,
        "test-video.mp4",
        "video/mp4",
        &test_content,
    )
    .await;

    assert!(result.is_ok(), "Video upload should succeed");
    let upload_result = result.unwrap();

    assert!(!upload_result.file_url.is_empty(), "Should have file URL");

    println!("✅ Video upload workflow completed");
}

#[tokio::test]
async fn test_video_format_validation() {
    let app = spawn_app().await;
    let client = Client::new();
    let test_user = create_test_user_and_login(&app.address).await;

    println!("🎥 Testing video format validation");

    let test_content = b"fake video";
    let mut hasher = Sha256::new();
    hasher.update(test_content);
    let test_hash = format!("{:x}", hasher.finalize());

    // Test allowed video formats
    let allowed_formats = vec!["mp4", "mov", "m4v", "3gp", "webm"];

    for format in allowed_formats {
        let response = client
            .post(&format!("{}/media/upload-url", &app.address))
            .header("Authorization", format!("Bearer {}", test_user.token))
            .json(&json!({
                "filename": format!("test.{}", format),
                "content_type": format!("video/{}", format),
                "expected_hash": test_hash,
                "file_size": test_content.len()
            }))
            .send()
            .await
            .expect("Failed to request upload URL");

        assert_eq!(response.status(), 200, "Should accept .{} format", format);
    }

    // Test invalid format
    let response = client
        .post(&format!("{}/media/upload-url", &app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&json!({
            "filename": "test.wmv",
            "content_type": "video/wmv",
            "expected_hash": test_hash,
            "file_size": test_content.len()
        }))
        .send()
        .await
        .expect("Failed to request upload URL");

    assert_eq!(response.status(), 400, "Should reject .wmv format");

    println!("✅ Video format validation passed");
}

#[tokio::test]
async fn test_video_size_validation() {
    let app = spawn_app().await;
    let client = Client::new();
    let test_user = create_test_user_and_login(&app.address).await;

    println!("🎥 Testing video size validation");

    let mut hasher = Sha256::new();
    hasher.update(b"test");
    let test_hash = format!("{:x}", hasher.finalize());

    // Try to upload video larger than 100MB limit
    let large_size = 101 * 1024 * 1024; // 101 MB

    let response = client
        .post(&format!("{}/media/upload-url", &app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&json!({
            "filename": "large-video.mp4",
            "content_type": "video/mp4",
            "expected_hash": test_hash,
            "file_size": large_size
        }))
        .send()
        .await
        .expect("Failed to request upload URL");

    assert_eq!(response.status(), 400, "Should reject video over 100MB");

    let error_data: serde_json::Value = response.json().await.expect("Failed to parse error");
    assert!(error_data["error"].as_str().unwrap().contains("too large"));

    println!("✅ Video size validation passed");
}

// ============================================================================
// UTILITY TESTS - URL Generation
// ============================================================================

#[tokio::test]
async fn test_signed_url_generation() {
    let app = spawn_app().await;
    let client = Client::new();
    let test_user = create_test_user_and_login(&app.address).await;

    println!("🔐 Testing signed URL generation");

    let test_content = b"test";
    let mut hasher = Sha256::new();
    hasher.update(test_content);
    let test_hash = format!("{:x}", hasher.finalize());

    let response = client
        .post(&format!("{}/media/upload-url", &app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&json!({
            "filename": "test.jpg",
            "content_type": "image/jpeg",
            "expected_hash": test_hash,
            "file_size": test_content.len()
        }))
        .send()
        .await
        .expect("Failed to request URL");

    assert_eq!(response.status(), 200);

    let data: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(data["data"]["upload_url"].is_string());
    assert!(data["data"]["object_key"].is_string());

    println!("✅ Signed URL generation works");
}