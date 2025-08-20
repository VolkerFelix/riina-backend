// Test workout media upload functionality with MinIO integration

use reqwest::Client;
use serde_json::json;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};
use common::workout_data_helpers::{WorkoutData, WorkoutType, upload_workout_data_for_user};

#[tokio::test]
async fn test_workout_media_upload_with_image() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing workout media upload with image");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;
    
    println!("✅ Test user created and logged in");
    
    // First, upload a workout to associate the media with
    let workout_data = WorkoutData::new_with_offset_hours(WorkoutType::Moderate, 1, 30);
    let workout_response = upload_workout_data_for_user(
        &client,
        &app.address,
        &token,
        &workout_data
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
    
    // Upload the image using multipart form
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(png_data.clone())
                .file_name("test_workout_image.png")
                .mime_str("image/png")
                .expect("Failed to set mime type")
        )
        .text("workout_id", workout_data.device_id.clone()); // Associate with workout if needed
    
    let upload_response = client
        .post(&format!("{}/health/upload_workout_media", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form)
        .send()
        .await
        .expect("Failed to execute upload request");
    
    let status = upload_response.status();
    let response_text = upload_response.text().await.expect("Failed to get response text");
    
    println!("Upload response status: {}", status);
    println!("Upload response: {}", response_text);
    
    assert_eq!(status, 200, "Media upload should succeed");
    
    // Parse the response to get the file URL
    let response_json: serde_json::Value = serde_json::from_str(&response_text)
        .expect("Failed to parse response JSON");
    
    assert!(response_json["success"].as_bool().unwrap_or(false), "Upload should be successful");
    assert!(response_json["data"]["file_url"].is_string(), "Response should contain file_url");
    
    let file_url = response_json["data"]["file_url"].as_str().unwrap();
    println!("✅ Image uploaded successfully to: {}", file_url);
    
    // Test retrieving the uploaded image
    // The file_url now has format: /health/workout-media/{user_id}/{filename}
    let retrieve_response = client
        .get(&format!("{}{}", &app.address, file_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to retrieve uploaded media");
    
    assert_eq!(retrieve_response.status(), 200, "Media retrieval should succeed");
    
    let content_type = retrieve_response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    
    assert_eq!(content_type, "image/png", "Retrieved media should have correct content type");
    
    let retrieved_bytes = retrieve_response.bytes().await.expect("Failed to get response bytes");
    assert!(!retrieved_bytes.is_empty(), "Retrieved file should not be empty");
    
    println!("✅ Image retrieved successfully from MinIO");
    
    // Test that unauthenticated users cannot access the media
    let unauthorized_response = client
        .get(&format!("{}{}", &app.address, file_url))
        .send()
        .await
        .expect("Failed to execute unauthorized request");
    
    assert_eq!(unauthorized_response.status(), 401, "Unauthenticated access should be denied");
    println!("✅ Unauthenticated access correctly denied");
    
    // Test that other authenticated users CAN access the media (per requirement)
    let other_user = create_test_user_and_login(&app.address).await;
    let other_token = &other_user.token;
    
    let other_user_response = client
        .get(&format!("{}{}", &app.address, file_url))
        .header("Authorization", format!("Bearer {}", other_token))
        .send()
        .await
        .expect("Failed to execute other user request");
    
    assert_eq!(other_user_response.status(), 200, "Other authenticated users should be able to access media");
    println!("✅ Other authenticated users can access media (as per requirement)");
    
    println!("✅ All workout media upload tests passed!");
}

#[tokio::test]
async fn test_workout_media_upload_validates_file_type() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing workout media upload file type validation");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;
    
    // Try to upload an invalid file type (e.g., a text file)
    let invalid_data = b"This is not an image file".to_vec();
    
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(invalid_data)
                .file_name("test.txt")
                .mime_str("text/plain")
                .expect("Failed to set mime type")
        );
    
    let upload_response = client
        .post(&format!("{}/health/upload_workout_media", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form)
        .send()
        .await
        .expect("Failed to execute upload request");
    
    assert_eq!(upload_response.status(), 400, "Invalid file type should be rejected");
    
    let response_json: serde_json::Value = upload_response.json().await
        .expect("Failed to parse response JSON");
    
    assert!(!response_json["success"].as_bool().unwrap_or(true), "Upload should fail");
    assert!(response_json["error"].as_str().unwrap_or("").contains("not allowed"), 
            "Error message should indicate file type not allowed");
    
    println!("✅ Invalid file type correctly rejected");
}

#[tokio::test]
async fn test_workout_media_upload_validates_empty_file() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing workout media upload empty file validation");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;
    
    // Try to upload an empty file
    let empty_data: Vec<u8> = vec![];
    
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(empty_data)
                .file_name("empty.jpg")
                .mime_str("image/jpeg")
                .expect("Failed to set mime type")
        );
    
    let upload_response = client
        .post(&format!("{}/health/upload_workout_media", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form)
        .send()
        .await
        .expect("Failed to execute upload request");
    
    assert_eq!(upload_response.status(), 400, "Empty file should be rejected");
    
    let response_json: serde_json::Value = upload_response.json().await
        .expect("Failed to parse response JSON");
    
    assert!(!response_json["success"].as_bool().unwrap_or(true), "Upload should fail");
    assert!(response_json["error"].as_str().unwrap_or("").contains("empty"), 
            "Error message should indicate file is empty");
    
    println!("✅ Empty file correctly rejected");
}

#[tokio::test]
async fn test_workout_media_upload_with_valid_video() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing workout media upload with video file");
    
    // Create test user and login
    let test_user = create_test_user_and_login(&app.address).await;
    let token = &test_user.token;
    
    // Create a minimal valid MP4 header (just enough to be recognized as MP4)
    // This is not a playable video but has the correct file signature
    let mp4_data: Vec<u8> = vec![
        0x00, 0x00, 0x00, 0x20, 0x66, 0x74, 0x79, 0x70, // ftyp box
        0x69, 0x73, 0x6F, 0x6D, 0x00, 0x00, 0x02, 0x00,
        0x69, 0x73, 0x6F, 0x6D, 0x69, 0x73, 0x6F, 0x32,
        0x61, 0x76, 0x63, 0x31, 0x6D, 0x70, 0x34, 0x31
    ];
    
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(mp4_data)
                .file_name("workout_video.mp4")
                .mime_str("video/mp4")
                .expect("Failed to set mime type")
        );
    
    let upload_response = client
        .post(&format!("{}/health/upload_workout_media", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form)
        .send()
        .await
        .expect("Failed to execute upload request");
    
    assert_eq!(upload_response.status(), 200, "Video upload should succeed");
    
    let response_json: serde_json::Value = upload_response.json().await
        .expect("Failed to parse response JSON");
    
    assert!(response_json["success"].as_bool().unwrap_or(false), "Video upload should be successful");
    assert_eq!(response_json["data"]["file_type"].as_str().unwrap_or(""), "video", 
               "File should be recognized as video");
    
    println!("✅ Video file uploaded successfully");
}