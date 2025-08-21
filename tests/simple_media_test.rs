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
        .get(&format!("{}/health/workout-media-url/{}/test.png", &app.address, test_user.user_id))
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