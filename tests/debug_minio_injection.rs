// Debug test to isolate MinIO service injection issue

use reqwest::Client;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};

#[tokio::test]
async fn debug_minio_service_extraction() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Debug: Testing MinIO service extraction");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;
    
    println!("✅ Test user created and logged in");
    
    // Try to access the upload endpoint with a simple GET request
    // This should give us a method not allowed, not a 500 error if the service is injected correctly
    let response = client
        .get(&format!("{}/health/upload_workout_media", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to execute request");
    
    let status = response.status();
    let response_text = response.text().await.expect("Failed to get response text");
    
    println!("GET response status: {}", status);
    println!("GET response body: {}", response_text);
    
    // A 405 Method Not Allowed indicates the route exists and services are injected correctly
    // A 500 Internal Server Error indicates a service injection problem
    if status == 500 {
        println!("❌ Service injection issue detected - 500 error on GET request");
    } else if status == 405 {
        println!("✅ Route exists and services likely injected correctly - got expected 405");
    } else {
        println!("ℹ️ Got unexpected status: {}", status);
    }
    
    // Now try with an empty POST to see what specific error we get
    let post_response = client
        .post(&format!("{}/health/upload_workout_media", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("Failed to execute POST request");
    
    let post_status = post_response.status();
    let post_text = post_response.text().await.expect("Failed to get POST response text");
    
    println!("POST response status: {}", post_status);
    println!("POST response body: {}", post_text);
    
    // Any response other than 500 "Requested application data is not configured correctly"
    // indicates the service injection is working
    if post_text.contains("Requested application data is not configured correctly") {
        println!("❌ Confirmed: MinIO service injection is failing");
        panic!("MinIO service injection failed");
    } else {
        println!("✅ MinIO service injection appears to be working");
    }
}