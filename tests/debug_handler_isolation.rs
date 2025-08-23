// Test to isolate which specific parameter is causing the injection issue

use reqwest::Client;
use serde_json::json;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};

// Let's create a temporary simplified endpoint to test each parameter individually

#[tokio::test] 
async fn debug_handler_parameter_isolation() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Debug: Testing which parameter causes injection failure");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;
    
    println!("✅ Test user created and logged in");
    
    // Test 1: Try to access a working endpoint with similar authentication to confirm auth works
    println!("🔬 Test 1: Testing authentication with a known working endpoint");
    let response1 = client
        .get(&format!("{}/health/activity_sum", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to execute auth test request");
    
    println!("Auth test - Status: {}, Body: {}", 
             response1.status(), 
             response1.text().await.unwrap_or_default());
    
    // Test 2: Try to access an endpoint that uses web::Data<MinIOService> but not multipart
    // Since we don't have such an endpoint, let's try the serve endpoint with a fake filename
    println!("🔬 Test 2: Testing MinIO service injection with serve endpoint");
    let response2 = client
        .get(&format!("{}/health/workout-media/nonexistent.png", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to execute serve test request");
    
    println!("MinIO service test - Status: {}, Body: {}", 
             response2.status(), 
             response2.text().await.unwrap_or_default());
    
    println!("🔬 Analysis complete");
}