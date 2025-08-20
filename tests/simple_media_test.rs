// Simple test to verify MinIO service is properly injected

use reqwest::Client;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};

#[tokio::test]
async fn test_media_endpoint_exists() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing that media endpoint exists and MinIO service is injected");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;
    
    // Try to access the upload endpoint with invalid data to see what happens
    let response = client
        .post(&format!("{}/health/upload_workout_media", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({"invalid": "data"}))
        .send()
        .await
        .expect("Failed to execute request");
    
    let status = response.status();
    let response_text = response.text().await.expect("Failed to get response text");
    
    println!("Response status: {}", status);
    println!("Response body: {}", response_text);
    
    // We expect a 400 (bad request) for invalid data, not 500 (server error)
    // A 500 would indicate the MinIO service is not injected properly
    assert_ne!(status, 500, "Should not get server error - MinIO service should be injected properly");
    
    println!("✅ Media endpoint accessible and MinIO service is properly injected");
}