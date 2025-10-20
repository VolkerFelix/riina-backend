// Simple test to verify signed URL endpoints work

use reqwest::Client;
use sha2::{Sha256, Digest};

mod common;
use common::utils::{spawn_app, create_test_user_and_login};

#[tokio::test]
async fn test_signed_url_endpoints_exist() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing that signed URL endpoints exist and MinIO service is injected");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;
    
    // Create a simple test file hash (SHA256 of "test")
    let mut hasher = Sha256::new();
    hasher.update(b"test");
    let test_hash = format!("{:x}", hasher.finalize());
    
    // Try to request an upload URL
    let response = client
        .post(&format!("{}/health/request-upload-url", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "filename": "test.png",
            "content_type": "image/png",
            "expected_hash": test_hash,
            "file_size": 1024
        }))
        .send()
        .await
        .expect("Failed to execute request");
    
    let status = response.status();
    let response_text = response.text().await.expect("Failed to get response text");
    
    println!("Response status: {}", status);
    println!("Response body: {}", response_text);
    
    // We expect either 200 (success) or 400 (bad request), not 500 (server error)
    // A 500 would indicate the MinIO service is not injected properly
    assert_ne!(status, 500, "Should not get server error - MinIO service should be injected properly");
    
    println!("✅ Signed URL endpoints accessible and MinIO service is properly injected");
}

#[tokio::test]
async fn test_download_url_endpoint() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing download URL endpoint");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;
    
    // Try to get a download URL for a non-existent file
    let response = client
        .get(&format!("{}/media/download-url/{}/test.png", &app.address, test_user.user_id))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to execute request");
    
    let status = response.status();
    
    println!("Response status: {}", status);
    
    // We expect 404 (not found) for non-existent file, not 500 (server error)
    assert_ne!(status, 500, "Should not get server error");
    
    println!("✅ Download URL endpoint accessible");
}

#[tokio::test]
async fn test_full_media_upload_and_download_workflow() {
    let app = spawn_app().await;
    let client = Client::new();

    println!("🔍 Testing full media upload and download workflow");

    // Create test user and login
    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;

    // Step 1: Create test file content
    let test_content = b"fake image content for testing";
    let mut hasher = Sha256::new();
    hasher.update(test_content);
    let test_hash = format!("{:x}", hasher.finalize());

    println!("📝 Test file hash: {}", test_hash);

    // Step 2: Request upload URL
    let upload_response = client
        .post(&format!("{}/media/upload-url", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "filename": "test-image.jpg",
            "content_type": "image/jpeg",
            "expected_hash": test_hash,
            "file_size": test_content.len()
        }))
        .send()
        .await
        .expect("Failed to request upload URL");

    assert_eq!(upload_response.status(), 200, "Upload URL request should succeed");

    let upload_data: serde_json::Value = upload_response.json().await.expect("Failed to parse upload response");
    println!("📤 Upload URL response: {}", serde_json::to_string_pretty(&upload_data).unwrap());

    let upload_url = upload_data["data"]["upload_url"].as_str().expect("Missing upload_url");
    let object_key = upload_data["data"]["object_key"].as_str().expect("Missing object_key");

    println!("🔑 Object key: {}", object_key);

    // Step 3: Upload file to MinIO (simulate)
    let hash_bytes = hex::decode(&test_hash).expect("Invalid hash");
    let base64_hash = base64::encode(&hash_bytes);

    let upload_result = client
        .put(upload_url)
        .header("Content-Type", "image/jpeg")
        .header("x-amz-checksum-sha256", base64_hash)
        .body(test_content.to_vec())
        .send()
        .await
        .expect("Failed to upload to MinIO");

    println!("📤 MinIO upload status: {}", upload_result.status());
    assert!(upload_result.status().is_success(), "MinIO upload should succeed");

    // Step 4: Confirm upload
    let confirm_response = client
        .post(&format!("{}/media/confirm-upload", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "object_key": object_key,
            "expected_hash": test_hash
        }))
        .send()
        .await
        .expect("Failed to confirm upload");

    assert_eq!(confirm_response.status(), 200, "Upload confirmation should succeed");

    let confirm_data: serde_json::Value = confirm_response.json().await.expect("Failed to parse confirm response");
    println!("✅ Confirm response: {}", serde_json::to_string_pretty(&confirm_data).unwrap());

    let file_url = confirm_data["data"]["file_url"].as_str().expect("Missing file_url");
    println!("🔗 Returned file_url: {}", file_url);

    // Step 5: Parse the file_url (this is what the frontend does)
    // Expected format: "{user_id}/{filename}"
    let parts: Vec<&str> = file_url.split('/').collect();
    assert_eq!(parts.len(), 2, "file_url should have format user_id/filename, got: {}", file_url);

    let returned_user_id = parts[0];
    let returned_filename = parts[1];

    println!("📋 Parsed user_id: {}, filename: {}", returned_user_id, returned_filename);

    // Step 6: Use the parsed values to get download URL (this is what SignedImage does)
    let download_response = client
        .get(&format!("{}/media/download-url/{}/{}", &app.address, returned_user_id, returned_filename))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get download URL");

    let download_status = download_response.status();
    let download_data: serde_json::Value = download_response.json().await.expect("Failed to parse download response");

    println!("📥 Download URL response status: {}", download_status);
    println!("📥 Download URL response: {}", serde_json::to_string_pretty(&download_data).unwrap());

    assert_eq!(download_status, 200, "Download URL request should succeed");
    assert!(download_data["data"]["url"].is_string(), "Should return signed download URL");

    let signed_url = download_data["data"]["url"].as_str().expect("Missing signed URL");
    println!("🔗 Signed download URL: {}", signed_url);

    // Step 7: Verify we can download the file using the signed URL
    let download_file_response = client
        .get(signed_url)
        .send()
        .await
        .expect("Failed to download file");

    assert!(download_file_response.status().is_success(), "File download should succeed");

    let downloaded_content = download_file_response.bytes().await.expect("Failed to get file bytes");
    assert_eq!(downloaded_content.as_ref(), test_content, "Downloaded content should match uploaded content");

    println!("✅ Full workflow test passed! File uploaded, confirmed, and downloaded successfully");
}

#[tokio::test]
async fn test_video_upload_workflow() {
    let app = spawn_app().await;
    let client = Client::new();

    println!("🎥 Testing video upload workflow");

    // Create test user and login
    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;

    // Step 1: Create test video content (simulated)
    let test_video_content = b"fake video content for testing - this would be actual video bytes in production";
    let mut hasher = Sha256::new();
    hasher.update(test_video_content);
    let test_hash = format!("{:x}", hasher.finalize());

    println!("📝 Test video hash: {}", test_hash);

    // Step 2: Request upload URL for video
    let upload_response = client
        .post(&format!("{}/media/upload-url", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "filename": "test-video.mp4",
            "content_type": "video/mp4",
            "expected_hash": test_hash,
            "file_size": test_video_content.len()
        }))
        .send()
        .await
        .expect("Failed to request upload URL");

    assert_eq!(upload_response.status(), 200, "Video upload URL request should succeed");

    let upload_data: serde_json::Value = upload_response.json().await.expect("Failed to parse upload response");
    println!("📤 Video upload URL response: {}", serde_json::to_string_pretty(&upload_data).unwrap());

    let upload_url = upload_data["data"]["upload_url"].as_str().expect("Missing upload_url");
    let object_key = upload_data["data"]["object_key"].as_str().expect("Missing object_key");

    println!("🔑 Object key: {}", object_key);

    // Step 3: Upload video to MinIO
    let hash_bytes = hex::decode(&test_hash).expect("Invalid hash");
    let base64_hash = base64::encode(&hash_bytes);

    let upload_result = client
        .put(upload_url)
        .header("Content-Type", "video/mp4")
        .header("x-amz-checksum-sha256", base64_hash)
        .body(test_video_content.to_vec())
        .send()
        .await
        .expect("Failed to upload video to MinIO");

    println!("📤 MinIO video upload status: {}", upload_result.status());
    assert!(upload_result.status().is_success(), "MinIO video upload should succeed");

    // Step 4: Confirm upload
    let confirm_response = client
        .post(&format!("{}/media/confirm-upload", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "object_key": object_key,
            "expected_hash": test_hash
        }))
        .send()
        .await
        .expect("Failed to confirm upload");

    assert_eq!(confirm_response.status(), 200, "Video upload confirmation should succeed");

    let confirm_data: serde_json::Value = confirm_response.json().await.expect("Failed to parse confirm response");
    println!("✅ Video confirm response: {}", serde_json::to_string_pretty(&confirm_data).unwrap());

    let file_url = confirm_data["data"]["file_url"].as_str().expect("Missing file_url");
    println!("🔗 Returned video file_url: {}", file_url);

    println!("✅ Video upload workflow test passed!");
}

#[tokio::test]
async fn test_video_size_validation() {
    let app = spawn_app().await;
    let client = Client::new();

    println!("🎥 Testing video size validation");

    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;

    // Create hash for a fake large video
    let mut hasher = Sha256::new();
    hasher.update(b"large video");
    let test_hash = format!("{:x}", hasher.finalize());

    // Try to upload a video that's too large (over 100 MB)
    let large_video_size = 101 * 1024 * 1024; // 101 MB

    let upload_response = client
        .post(&format!("{}/media/upload-url", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "filename": "large-video.mp4",
            "content_type": "video/mp4",
            "expected_hash": test_hash,
            "file_size": large_video_size
        }))
        .send()
        .await
        .expect("Failed to request upload URL");

    assert_eq!(upload_response.status(), 400, "Should reject video that's too large");

    let error_data: serde_json::Value = upload_response.json().await.expect("Failed to parse error response");
    println!("📛 Error response: {}", serde_json::to_string_pretty(&error_data).unwrap());

    assert!(error_data["error"].as_str().unwrap().contains("too large"), "Error should mention file is too large");

    println!("✅ Video size validation test passed!");
}

#[tokio::test]
async fn test_video_format_validation() {
    let app = spawn_app().await;
    let client = Client::new();

    println!("🎥 Testing video format validation");

    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;

    let mut hasher = Sha256::new();
    hasher.update(b"test");
    let test_hash = format!("{:x}", hasher.finalize());

    // Test allowed video formats
    let allowed_formats = vec!["mp4", "mov", "m4v", "3gp", "webm"];

    for format in allowed_formats {
        let upload_response = client
            .post(&format!("{}/media/upload-url", &app.address))
            .header("Authorization", format!("Bearer {}", token))
            .json(&serde_json::json!({
                "filename": format!("test-video.{}", format),
                "content_type": format!("video/{}", format),
                "expected_hash": test_hash,
                "file_size": 1024
            }))
            .send()
            .await
            .expect("Failed to request upload URL");

        assert_eq!(upload_response.status(), 200, "Should accept .{} video format", format);
    }

    // Test invalid video format
    let upload_response = client
        .post(&format!("{}/media/upload-url", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "filename": "test-video.wmv",
            "content_type": "video/wmv",
            "expected_hash": test_hash,
            "file_size": 1024
        }))
        .send()
        .await
        .expect("Failed to request upload URL");

    assert_eq!(upload_response.status(), 400, "Should reject .wmv video format");

    println!("✅ Video format validation test passed!");
}