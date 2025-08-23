// Debug test to isolate exactly where the multipart form processing fails

use reqwest::Client;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};

#[tokio::test]
async fn debug_multipart_form_processing() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Debug: Testing multipart form processing step by step");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;
    
    println!("✅ Test user created and logged in");
    
    // Create a minimal PNG file
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
    
    // Test with different multipart configurations
    
    // Test 1: Minimal multipart form
    println!("🔬 Test 1: Minimal multipart form");
    let form1 = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(png_data.clone())
                .file_name("test.png")
                .mime_str("image/png")
                .expect("Failed to set mime type")
        );
    
    let response1 = client
        .post(&format!("{}/health/upload_workout_media", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form1)
        .send()
        .await
        .expect("Failed to execute request 1");
    
    println!("Response 1 - Status: {}, Body: {}", 
             response1.status(), 
             response1.text().await.unwrap_or_default());
    
    // Test 2: With workout_id field
    println!("🔬 Test 2: With workout_id field");
    let form2 = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(png_data.clone())
                .file_name("test.png")
                .mime_str("image/png")
                .expect("Failed to set mime type")
        )
        .text("workout_id", "test-workout-123");
    
    let response2 = client
        .post(&format!("{}/health/upload_workout_media", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form2)
        .send()
        .await
        .expect("Failed to execute request 2");
    
    println!("Response 2 - Status: {}, Body: {}", 
             response2.status(), 
             response2.text().await.unwrap_or_default());
    
    // Test 3: Empty multipart form
    println!("🔬 Test 3: Empty multipart form");
    let form3 = reqwest::multipart::Form::new();
    
    let response3 = client
        .post(&format!("{}/health/upload_workout_media", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form3)
        .send()
        .await
        .expect("Failed to execute request 3");
    
    println!("Response 3 - Status: {}, Body: {}", 
             response3.status(), 
             response3.text().await.unwrap_or_default());
}