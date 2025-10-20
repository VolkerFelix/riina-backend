//! Post videos integration tests
//!
//! Tests for video functionality in posts:
//! - Creating posts with video URLs
//! - Uploading videos and attaching to posts
//! - Retrieving posts with videos
//! - Updating post video URLs
//! - Multiple videos per post

use reqwest::Client;
use serde_json::json;
use sha2::{Sha256, Digest};

mod common;
use common::utils::{spawn_app, create_test_user_and_login, delete_test_user};
use common::admin_helpers::create_admin_user_and_login;

#[tokio::test]
async fn test_create_post_with_video_urls() {
    let app = spawn_app().await;
    let client = Client::new();

    println!("🎥 Testing post creation with video URLs");

    let test_user = create_test_user_and_login(&app.address).await;
    let admin_user = create_admin_user_and_login(&app.address).await;
    let token = &test_user.token;

    // Create a universal post with video URLs
    let video_urls = vec![
        format!("{}/test-video1.mp4", test_user.user_id),
        format!("{}/test-video2.mp4", test_user.user_id),
    ];

    let post_response = client
        .post(&format!("{}/posts/", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "post_type": "universal",
            "content": "Check out my workout videos!",
            "video_urls": video_urls,
            "visibility": "public"
        }))
        .send()
        .await
        .expect("Failed to create post");

    assert_eq!(post_response.status(), 200, "Post creation should succeed");

    let post_data: serde_json::Value = post_response.json().await.expect("Failed to parse response");
    println!("✅ Post created: {}", serde_json::to_string_pretty(&post_data).unwrap());

    let post_id = post_data["data"]["id"].as_str().expect("Missing post ID");

    // Retrieve the post and verify video URLs
    let get_response = client
        .get(&format!("{}/posts/{}", &app.address, post_id))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get post");

    assert_eq!(get_response.status(), 200, "Get post should succeed");

    let get_data: serde_json::Value = get_response.json().await.expect("Failed to parse response");
    let retrieved_video_urls = get_data["data"]["video_urls"]
        .as_array()
        .expect("Should have video_urls array");

    assert_eq!(retrieved_video_urls.len(), 2, "Should have 2 video URLs");
    assert_eq!(retrieved_video_urls[0].as_str().unwrap(), video_urls[0]);
    assert_eq!(retrieved_video_urls[1].as_str().unwrap(), video_urls[1]);

    println!("✅ Post with video URLs created and retrieved successfully");

    // Cleanup
    delete_test_user(&app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_full_video_upload_and_post_workflow() {
    let app = spawn_app().await;
    let client = Client::new();

    println!("🎥 Testing full video upload and post creation workflow");

    let test_user = create_test_user_and_login(&app.address).await;
    let admin_user = create_admin_user_and_login(&app.address).await;
    let token = &test_user.token;

    // Step 1: Upload a video file
    let test_video_content = b"fake video content for testing";
    let mut hasher = Sha256::new();
    hasher.update(test_video_content);
    let test_hash = format!("{:x}", hasher.finalize());

    // Request upload URL
    let upload_response = client
        .post(&format!("{}/media/upload-url", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "filename": "workout-video.mp4",
            "content_type": "video/mp4",
            "expected_hash": test_hash,
            "file_size": test_video_content.len()
        }))
        .send()
        .await
        .expect("Failed to request upload URL");

    assert_eq!(upload_response.status(), 200, "Upload URL request should succeed");

    let upload_data: serde_json::Value = upload_response.json().await.expect("Failed to parse upload response");
    let upload_url = upload_data["data"]["upload_url"].as_str().expect("Missing upload_url");
    let object_key = upload_data["data"]["object_key"].as_str().expect("Missing object_key");

    // Upload to MinIO
    use base64::{Engine as _, engine::general_purpose};
    let hash_bytes = hex::decode(&test_hash).expect("Invalid hash");
    let base64_hash = general_purpose::STANDARD.encode(&hash_bytes);

    let upload_result = client
        .put(upload_url)
        .header("Content-Type", "video/mp4")
        .header("x-amz-checksum-sha256", base64_hash)
        .body(test_video_content.to_vec())
        .send()
        .await
        .expect("Failed to upload to MinIO");

    assert!(upload_result.status().is_success(), "MinIO upload should succeed");

    // Confirm upload
    let confirm_response = client
        .post(&format!("{}/media/confirm-upload", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "object_key": object_key,
            "expected_hash": test_hash
        }))
        .send()
        .await
        .expect("Failed to confirm upload");

    assert_eq!(confirm_response.status(), 200, "Upload confirmation should succeed");

    let confirm_data: serde_json::Value = confirm_response.json().await.expect("Failed to parse confirm response");
    let video_url = confirm_data["data"]["file_url"].as_str().expect("Missing file_url");

    println!("🔗 Video uploaded: {}", video_url);

    // Step 2: Create a post with the uploaded video
    let post_response = client
        .post(&format!("{}/posts/", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "post_type": "universal",
            "content": "My awesome workout video!",
            "video_urls": vec![video_url],
            "visibility": "public"
        }))
        .send()
        .await
        .expect("Failed to create post");

    assert_eq!(post_response.status(), 200, "Post creation should succeed");

    let post_data: serde_json::Value = post_response.json().await.expect("Failed to parse post response");
    let post_id = post_data["data"]["id"].as_str().expect("Missing post ID");

    println!("✅ Post created with video: {}", post_id);

    // Step 3: Retrieve the post and verify video
    let get_response = client
        .get(&format!("{}/posts/{}", &app.address, post_id))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get post");

    assert_eq!(get_response.status(), 200, "Get post should succeed");

    let get_data: serde_json::Value = get_response.json().await.expect("Failed to parse response");
    let retrieved_video_urls = get_data["data"]["video_urls"]
        .as_array()
        .expect("Should have video_urls array");

    assert_eq!(retrieved_video_urls.len(), 1, "Should have 1 video URL");
    assert_eq!(retrieved_video_urls[0].as_str().unwrap(), video_url);

    println!("✅ Full video upload and post workflow test passed!");

    // Cleanup
    delete_test_user(&app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_update_post_video_urls() {
    let app = spawn_app().await;
    let client = Client::new();

    println!("🎥 Testing post video URL updates");

    let test_user = create_test_user_and_login(&app.address).await;
    let admin_user = create_admin_user_and_login(&app.address).await;
    let token = &test_user.token;

    // Create a post with one video
    let initial_video = format!("{}/video1.mp4", test_user.user_id);

    let post_response = client
        .post(&format!("{}/posts/", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "post_type": "universal",
            "content": "Initial video",
            "video_urls": vec![initial_video],
            "visibility": "public"
        }))
        .send()
        .await
        .expect("Failed to create post");

    assert_eq!(post_response.status(), 200);

    let post_data: serde_json::Value = post_response.json().await.expect("Failed to parse response");
    let post_id = post_data["data"]["id"].as_str().expect("Missing post ID");

    // Update the post with new videos
    let new_videos = vec![
        format!("{}/video2.mp4", test_user.user_id),
        format!("{}/video3.mp4", test_user.user_id),
    ];

    let update_response = client
        .patch(&format!("{}/posts/{}", &app.address, post_id))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "video_urls": new_videos,
            "content": "Updated with new videos!"
        }))
        .send()
        .await
        .expect("Failed to update post");

    assert_eq!(update_response.status(), 200, "Post update should succeed");

    // Verify the updated videos
    let get_response = client
        .get(&format!("{}/posts/{}", &app.address, post_id))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get post");

    let get_data: serde_json::Value = get_response.json().await.expect("Failed to parse response");
    let updated_video_urls = get_data["data"]["video_urls"]
        .as_array()
        .expect("Should have video_urls array");

    assert_eq!(updated_video_urls.len(), 2, "Should have 2 updated video URLs");
    assert_eq!(updated_video_urls[0].as_str().unwrap(), new_videos[0]);
    assert_eq!(updated_video_urls[1].as_str().unwrap(), new_videos[1]);

    println!("✅ Post video URL update test passed!");

    // Cleanup
    delete_test_user(&app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_post_with_mixed_media() {
    let app = spawn_app().await;
    let client = Client::new();

    println!("🎥📸 Testing post with both images and videos");

    let test_user = create_test_user_and_login(&app.address).await;
    let admin_user = create_admin_user_and_login(&app.address).await;
    let token = &test_user.token;

    // Create a post with both images and videos
    let image_urls = vec![
        format!("{}/image1.jpg", test_user.user_id),
        format!("{}/image2.jpg", test_user.user_id),
    ];

    let video_urls = vec![
        format!("{}/video1.mp4", test_user.user_id),
    ];

    let post_response = client
        .post(&format!("{}/posts/", &app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "post_type": "universal",
            "content": "Mixed media post!",
            "image_urls": image_urls,
            "video_urls": video_urls,
            "visibility": "public"
        }))
        .send()
        .await
        .expect("Failed to create post");

    assert_eq!(post_response.status(), 200, "Post creation should succeed");

    let post_data: serde_json::Value = post_response.json().await.expect("Failed to parse response");
    let post_id = post_data["data"]["id"].as_str().expect("Missing post ID");

    // Retrieve and verify
    let get_response = client
        .get(&format!("{}/posts/{}", &app.address, post_id))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get post");

    let get_data: serde_json::Value = get_response.json().await.expect("Failed to parse response");

    let retrieved_images = get_data["data"]["image_urls"]
        .as_array()
        .expect("Should have image_urls array");
    let retrieved_videos = get_data["data"]["video_urls"]
        .as_array()
        .expect("Should have video_urls array");

    assert_eq!(retrieved_images.len(), 2, "Should have 2 image URLs");
    assert_eq!(retrieved_videos.len(), 1, "Should have 1 video URL");

    println!("✅ Mixed media post test passed!");

    // Cleanup
    delete_test_user(&app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&app.address, &admin_user.token, admin_user.user_id).await;
}
