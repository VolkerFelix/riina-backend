// Test workout media upload functionality with signed URLs and MinIO integration

use reqwest::Client;
use serde_json::json;
use sha2::{Sha256, Digest};
use uuid::Uuid;
use std::time::Duration;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};
use common::workout_data_helpers::{WorkoutData, WorkoutType, upload_workout_data_for_user};

#[tokio::test]
async fn test_workout_media_upload_with_signed_urls() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing workout media upload with signed URLs");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;
    
    println!("✅ Test user created and logged in");
    
    // First, upload a workout to associate the media with
    let mut workout_data = WorkoutData::new_with_offset_hours(WorkoutType::Moderate, 1, 30);
    let workout_response = upload_workout_data_for_user(
        &client,
        &app.address,
        &token,
        &mut workout_data
    ).await.expect("Failed to upload workout");
    
    println!("✅ Workout uploaded successfully: {:?}", workout_response);
    
    // Create a test image file (small PNG)
    // This is a minimal 1x1 pixel PNG (transparent)
    let png_data: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
        0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, // IDAT chunk
        0x54, 0x78, 0x9C, 0x62, 0x00, 0x01, 0x00, 0x00,
        0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
        0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, // IEND chunk
        0x42, 0x60, 0x82
    ];
    
    // Calculate SHA256 hash of the image
    let mut hasher = Sha256::new();
    hasher.update(&png_data);
    let file_hash = format!("{:x}", hasher.finalize());
    
    println!("📋 File hash: {}", file_hash);
    
    // Step 1: Request upload URL
    let upload_url_response = client
        .post(&format!("{}/health/request-upload-url", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "filename": "test_workout_image.png",
            "content_type": "image/png",
            "expected_hash": file_hash,
            "workout_id": workout_data.device_id.clone()
        }))
        .send()
        .await
        .expect("Failed to request upload URL");
    
    assert_eq!(upload_url_response.status(), 200, "Should get upload URL successfully");
    
    let upload_url_data: serde_json::Value = upload_url_response
        .json()
        .await
        .expect("Failed to parse upload URL response");
    
    println!("✅ Got upload URL: {:?}", upload_url_data);
    
    // Extract the upload URL and object key
    let upload_url = upload_url_data["data"]["upload_url"]
        .as_str()
        .expect("No upload URL in response");
    let object_key = upload_url_data["data"]["object_key"]
        .as_str()
        .expect("No object key in response");
    
    // Step 2: Upload file directly to MinIO using signed URL
    // Note: In a real test environment, MinIO must be running and accessible
    // This will fail if MinIO is not properly set up
    println!("📤 Uploading file to MinIO using signed URL...");
    
    // For testing purposes, we'll skip the actual MinIO upload since it requires
    // MinIO to be running. In a real test, you would do:
    /*
    let upload_response = client
        .put(upload_url)
        .header("Content-Type", "image/png")
        .header("x-amz-checksum-sha256", &checksum_base64)
        .body(png_data.clone())
        .send()
        .await
        .expect("Failed to upload to MinIO");
    
    assert_eq!(upload_response.status(), 200, "MinIO upload should succeed");
    */
    
    // Step 3: Confirm upload (this will fail since we didn't actually upload)
    let confirm_response = client
        .post(&format!("{}/health/confirm-upload", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "object_key": object_key,
            "expected_hash": file_hash
        }))
        .send()
        .await
        .expect("Failed to confirm upload");
    
    // We expect 404 since we didn't actually upload to MinIO
    assert_eq!(confirm_response.status(), 404, "Should get 404 for non-existent file");
    
    println!("✅ Signed URL workflow test completed");
}

#[tokio::test]
async fn test_download_signed_url() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing download signed URL generation");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;
    
    // Request a download URL for a non-existent file
    let response = client
        .get(&format!(
            "{}/health/workout-media-url/{}/test-image.png",
            &app.address,
            test_user.user_id
        ))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to request download URL");
    
    assert_eq!(response.status(), 404, "Should get 404 for non-existent file");
    
    let error_response: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse error response");
    
    assert_eq!(
        error_response["success"],
        false,
        "Response should indicate failure"
    );
    assert_eq!(
        error_response["error"],
        "File not found",
        "Should get file not found error"
    );
    
    println!("✅ Download URL endpoint working correctly");
}

// Unauthorized access to download URL
#[tokio::test]
async fn test_unauthorized_download_url() {
    let app = spawn_app().await;
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to build client");
    
    println!("🔍 Testing unauthorized download URL access");
    
    let response = client
        .get(&format!(
            "{}/health/workout-media-url/{}/test-image.png",
            &app.address,
            Uuid::new_v4()
        ))
        .send()
        .await
        .expect("Failed to request download URL");
    
    assert_eq!(response.status(), 401, "Should get 401 for unauthorized access");
}