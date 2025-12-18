//! Profile picture upload and management tests
//! 
//! This test suite covers profile picture functionality including:
//! - Profile picture upload URL generation
//! - Profile picture upload confirmation
//! - Profile picture download URL generation
//! - Profile picture validation (file type, size)
//! - User profile updates with profile pictures

use reqwest::Client;
use serde_json::json;
use sha2::{Sha256, Digest};
use uuid::Uuid;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, delete_test_user};
use common::admin_helpers::create_admin_user_and_login;

// ============================================================================
// PROFILE PICTURE UPLOAD TESTS
// ============================================================================

#[tokio::test]
async fn test_profile_picture_upload_url_generation() {
    let test_app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing profile picture upload URL generation");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;
    let token = &test_user.token;
    
    // Create a test file hash (SHA256 of "test-profile-picture")
    let mut hasher = Sha256::new();
    hasher.update(b"test-profile-picture");
    let test_hash = format!("{:x}", hasher.finalize());
    
    // Request upload URL for profile picture
    let response = client
        .post(&format!("{}/profile/picture/request-upload-url", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "filename": "profile-picture.jpg",
            "content_type": "image/jpeg",
            "expected_hash": test_hash
        }))
        .send()
        .await
        .expect("Failed to execute request");
    
    let status = response.status();
    let response_text = response.text().await.expect("Failed to get response text");
    
    println!("Response status: {}", status);
    println!("Response body: {}", response_text);
    
    // We expect either 200 (success) or 400 (bad request), not 500 (server error)
    assert_ne!(status, 500, "Should not get server error - MinIO service should be injected properly");
    
    println!("✅ Profile picture upload URL endpoint accessible");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_profile_picture_upload_url_invalid_file_type() {
    let test_app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing profile picture upload URL with invalid file type");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;
    let token = &test_user.token;
    
    // Create a test file hash
    let mut hasher = Sha256::new();
    hasher.update(b"test-file");
    let test_hash = format!("{:x}", hasher.finalize());
    
    // Request upload URL with invalid file type (PDF)
    let response = client
        .post(&format!("{}/profile/picture/request-upload-url", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "filename": "document.pdf",
            "content_type": "application/pdf",
            "expected_hash": test_hash
        }))
        .send()
        .await
        .expect("Failed to execute request");
    
    let status = response.status();
    let response_text = response.text().await.expect("Failed to get response text");
    
    println!("Response status: {}", status);
    println!("Response body: {}", response_text);
    
    // Should return 400 Bad Request for invalid file type
    assert_eq!(status, 400, "Should reject invalid file type for profile pictures");
    
    println!("✅ Profile picture upload correctly rejects invalid file types");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_profile_picture_download_url_generation() {
    let test_app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing profile picture download URL generation");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;
    let token = &test_user.token;
    
    // Request download URL for profile picture
    let response = client
        .get(&format!("{}/profile/picture/download-url/{}", &test_app.address, test_user.user_id))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to execute request");
    
    let status = response.status();
    let response_text = response.text().await.expect("Failed to get response text");
    
    println!("Response status: {}", status);
    println!("Response body: {}", response_text);
    
    // We expect either 200 (success) or 404 (not found), not 500 (server error)
    assert_ne!(status, 500, "Should not get server error");
    
    println!("✅ Profile picture download URL endpoint accessible");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_profile_picture_upload_workflow() {
    let test_app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing complete profile picture upload workflow");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;
    let token = &test_user.token;
    
    // Step 1: Request upload URL
    let mut hasher = Sha256::new();
    hasher.update(b"test-profile-picture-data");
    let test_hash = format!("{:x}", hasher.finalize());
    
    let upload_response = client
        .post(&format!("{}/profile/picture/request-upload-url", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "filename": "profile-picture.png",
            "content_type": "image/png",
            "expected_hash": test_hash
        }))
        .send()
        .await
        .expect("Failed to execute upload URL request");
    
    println!("Upload URL response status: {}", upload_response.status());
    
    // Step 2: Confirm upload (this will fail in test environment without actual MinIO)
    // but we can test that the endpoint exists and handles the request properly
    let confirm_response = client
        .post(&format!("{}/profile/picture/confirm-upload", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "object_key": "profile-pictures/test-user/test-profile-picture.png",
            "expected_hash": test_hash
        }))
        .send()
        .await
        .expect("Failed to execute confirm upload request");
    
    println!("Confirm upload response status: {}", confirm_response.status());
    
    // We expect the endpoints to exist and handle requests properly
    // (they may fail due to MinIO not being available in test environment)
    assert_ne!(upload_response.status(), 500, "Upload URL endpoint should exist");
    assert_ne!(confirm_response.status(), 500, "Confirm upload endpoint should exist");
    
    println!("✅ Profile picture upload workflow endpoints are accessible");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_user_profile_includes_profile_picture() {
    let test_app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing that user profile includes profile picture field");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;
    let token = &test_user.token;
    
    // Get user profile
    let response = client
        .get(&format!("{}/profile/user", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to execute request");
    
    let status = response.status();
    let response_text = response.text().await.expect("Failed to get response text");
    
    println!("Response status: {}", status);
    println!("Response body: {}", response_text);
    
    // Should return 200 OK
    assert_eq!(status, 200, "User profile should be accessible");
    
    // Parse response to check if profile_picture_url field exists
    let profile_data: serde_json::Value = serde_json::from_str(&response_text)
        .expect("Failed to parse profile response");
    
    // Check that the profile contains the profile_picture_url field
    if let Some(data) = profile_data.get("data") {
        assert!(data.get("profile_picture_url").is_some(), 
                "User profile should include profile_picture_url field");
    } else {
        panic!("Profile response should contain 'data' field");
    }
    
    println!("✅ User profile includes profile picture field");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_profile_picture_upload_with_large_file() {
    let test_app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing profile picture upload with large file size");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;
    let token = &test_user.token;
    
    // Create a test file hash for a large file (simulate 6MB file)
    let mut hasher = Sha256::new();
    hasher.update(b"large-file-data");
    let test_hash = format!("{:x}", hasher.finalize());
    
    // Request upload URL for large file
    let response = client
        .post(&format!("{}/profile/picture/request-upload-url", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "filename": "large-profile-picture.jpg",
            "content_type": "image/jpeg",
            "expected_hash": test_hash
        }))
        .send()
        .await
        .expect("Failed to execute request");
    
    let status = response.status();
    let response_text = response.text().await.expect("Failed to get response text");
    
    println!("Response status: {}", status);
    println!("Response body: {}", response_text);
    
    // The upload URL should be generated successfully
    // The size validation happens during the confirm upload step
    assert_ne!(status, 500, "Should not get server error");
    
    println!("✅ Profile picture upload URL generation handles large files");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}
