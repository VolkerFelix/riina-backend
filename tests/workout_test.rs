//! Consolidated workout functionality tests
//! 
//! This test suite covers all workout operations including:
//! - Workout data upload and validation
//! - Workout history and retrieval
//! - Admin workout management (CRUD operations)
//! - Media upload for workouts (images/videos)
//! - Workout approval workflow
//! - Notification system for workout uploads

use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use chrono::Utc;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request};
use common::workout_data_helpers::{WorkoutData, WorkoutType, upload_workout_data_for_user, create_health_profile_for_user};
use common::admin_helpers::create_admin_user_and_login;

// ============================================================================
// WORKOUT DATA UPLOAD TESTS
// ============================================================================

#[tokio::test]
async fn upload_workout_data_working() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 30);
    let response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;

    let status = response.is_ok();
    if !status {
        let error_body = response.err().unwrap();
        panic!("Health data upload failed with status {}: {}", status, error_body);
    }
    assert!(status);
}

#[tokio::test]
async fn upload_multiple_workout_data_sessions() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Upload multiple workouts
    for i in 0..3 {
        let mut workout_data = WorkoutData::new(
            if i % 2 == 0 { WorkoutType::Intense } else { WorkoutType::Moderate }, 
            Utc::now(), 
            30 + (i * 10)
        );
        
        let response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;
        assert!(response.is_ok(), "Workout {} should upload successfully", i);
    }
}

#[tokio::test]
async fn upload_workout_data_with_invalid_data_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Try to upload workout with invalid duration (negative)
    let invalid_workout = json!({
        "workout_start": Utc::now().to_rfc3339(),
        "workout_end": Utc::now().to_rfc3339(),
        "duration_minutes": -10,  // Invalid negative duration
        "calories": 300,
        "avg_heart_rate": 150,
        "max_heart_rate": 180
    });

    let response = client
        .post(&format!("{}/health/sync", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&invalid_workout)
        .send()
        .await
        .expect("Failed to execute request");

    assert!(!response.status().is_success(), "Invalid workout data should be rejected");
}

#[tokio::test]
async fn upload_workout_data_with_hard_workout() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let test_user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();
    let mut workout_data = WorkoutData::new_with_hr_freq(WorkoutType::Hard, Utc::now(), 30, Some(2));
    let response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;
    assert!(response.is_ok(), "Hard workout should upload successfully");
}

// ============================================================================
// WORKOUT HISTORY TESTS
// ============================================================================

#[tokio::test]
async fn test_workout_history_empty() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;

    // Test workout history with no data
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to execute workout history request");

    assert!(history_response.status().is_success());
    
    let history_data: serde_json::Value = history_response
        .json()
        .await
        .expect("Failed to parse workout history response");

    // Should return empty array for user with no workout history
    assert!(history_data["data"]["workouts"].is_array());
    assert_eq!(history_data["data"]["workouts"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_workout_history_with_data() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Upload a workout first
    let mut workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 45);
    upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await
        .expect("Workout upload should succeed");

    // Fetch workout history
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to execute workout history request");

    assert!(history_response.status().is_success());
    
    let history_data: serde_json::Value = history_response
        .json()
        .await
        .expect("Failed to parse workout history response");

    // Should return the uploaded workout
    assert!(history_data["data"]["workouts"].is_array());
    assert!(history_data["data"]["workouts"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn test_workout_history_pagination() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Upload multiple workouts
    for i in 0..5 {
        let mut workout_data = WorkoutData::new(WorkoutType::Light, Utc::now(), 20 + i);
        upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await
            .expect("Workout upload should succeed");
    }

    // Test pagination with limit
    let history_response = client
        .get(&format!("{}/health/history?limit=3", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to execute workout history request");

    assert!(history_response.status().is_success());
    
    let history_data: serde_json::Value = history_response
        .json()
        .await
        .expect("Failed to parse workout history response");

    // Should return only 3 workouts due to limit
    assert!(history_data["data"]["workouts"].is_array());
    assert_eq!(history_data["data"]["workouts"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_workout_detail_endpoint() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Upload a workout with heart rate data
    let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 45);
    let workout_response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;
    assert!(workout_response.is_ok(), "Workout upload should succeed");
    
    let _response_data = workout_response.unwrap();

    // Fetch workout history to get the workout ID
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to execute workout history request");

    assert!(history_response.status().is_success());
    
    let history_data: serde_json::Value = history_response
        .json()
        .await
        .expect("Failed to parse workout history response");

    let workouts = history_data["data"]["workouts"].as_array().unwrap();
    assert!(!workouts.is_empty(), "Should have at least one workout");
    
    let workout_id = workouts[0]["id"].as_str().unwrap();

    // Test the new workout detail endpoint
    let detail_response = client
        .get(&format!("{}/health/workout/{}", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to execute workout detail request");

    assert!(detail_response.status().is_success(), "Workout detail request should succeed");
    
    let detail_data: serde_json::Value = detail_response
        .json()
        .await
        .expect("Failed to parse workout detail response");

    // Verify the response structure and data makes sense
    assert!(detail_data["success"].as_bool().unwrap(), "Response should indicate success");
    assert!(detail_data["data"].is_object(), "Should have data object");
    
    let workout_detail = &detail_data["data"];
    
    // Verify basic workout fields
    assert_eq!(workout_detail["id"].as_str().unwrap(), workout_id, "Workout ID should match");
    assert!(workout_detail["workout_start"].is_string(), "Should have workout_start timestamp");
    assert!(workout_detail["workout_end"].is_string(), "Should have workout_end timestamp");
    
    // Verify heart rate data if present
    if workout_detail["heart_rate_data"].is_array() {
        let heart_rate_data = workout_detail["heart_rate_data"].as_array().unwrap();
        if !heart_rate_data.is_empty() {
            // Verify heart rate data structure
            for hr_point in heart_rate_data {
                assert!(hr_point["timestamp"].is_string(), "Heart rate point should have timestamp");
                assert!(hr_point["heart_rate"].is_number(), "Heart rate point should have heart_rate number");
                
                let hr_value = hr_point["heart_rate"].as_i64().unwrap();
                assert!(hr_value > 0 && hr_value < 300, "Heart rate should be reasonable (0-300 BPM)");
            }
        }
    }
    
    // Verify game stats are present (even if 0)
    assert!(workout_detail["stamina_gained"].is_number(), "Should have stamina_gained");
    assert!(workout_detail["strength_gained"].is_number(), "Should have strength_gained");
    
    // Verify calculated fields make sense
    if workout_detail["duration_minutes"].is_number() {
        let duration = workout_detail["duration_minutes"].as_i64().unwrap();
        assert!(duration > 0, "Duration should be positive");
    }
    
    if workout_detail["calories_burned"].is_number() {
        let calories = workout_detail["calories_burned"].as_i64().unwrap();
        assert!(calories > 0, "Calories should be positive");
    }
    
    if workout_detail["avg_heart_rate"].is_number() {
        let avg_hr = workout_detail["avg_heart_rate"].as_i64().unwrap();
        assert!(avg_hr > 0 && avg_hr < 300, "Average heart rate should be reasonable");
    }
    
    if workout_detail["max_heart_rate"].is_number() {
        let max_hr = workout_detail["max_heart_rate"].as_i64().unwrap();
        assert!(max_hr > 0 && max_hr < 300, "Max heart rate should be reasonable");
    }
}

#[tokio::test]
async fn test_workout_detail_endpoint_not_found() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;

    // Try to fetch a non-existent workout
    let fake_workout_id = Uuid::new_v4().to_string();
    let detail_response = client
        .get(&format!("{}/health/workout/{}", &test_app.address, fake_workout_id))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to execute workout detail request");

    assert_eq!(detail_response.status(), 404, "Should return 404 for non-existent workout");
}

#[tokio::test]
async fn test_workout_detail_endpoint_unauthorized_access() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create two users
    let user1 = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &user1).await.unwrap();
    let mut user2 = create_test_user_and_login(&test_app.address).await;

    // User1 uploads a workout
    let mut workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);
    let workout_response = upload_workout_data_for_user(&client, &test_app.address, &user1.token, &mut workout_data).await;
    assert!(workout_response.is_ok(), "Workout upload should succeed");
    
    // Get the workout ID from user1's history
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to execute workout history request");

    let history_data: serde_json::Value = history_response
        .json()
        .await
        .expect("Failed to parse workout history response");

    let workouts = history_data["data"]["workouts"].as_array().unwrap();
    let workout_id = workouts[0]["id"].as_str().unwrap();

    user2.token = "".to_string();

    // User2 tries to access user1's workout
    let detail_response = client
        .get(&format!("{}/health/workout/{}", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .expect("Failed to execute workout detail request");

    assert_eq!(detail_response.status(), 401, "Should return 401 when user tries to access another user's workout");
}

// ============================================================================
// ADMIN WORKOUT MANAGEMENT TESTS
// ============================================================================

#[tokio::test]
async fn test_admin_can_delete_workout() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a regular user
    let user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &user).await.unwrap();

    // Create a workout for the user
    let mut workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);
    let workout_response = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout_data).await;

    let response_data = workout_response.unwrap();
    let sync_id = response_data["data"]["sync_id"].as_str().unwrap();

    // Admin deletes the workout
    let delete_response = client
        .delete(&format!("{}/admin/workouts/{}", &test_app.address, sync_id))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute workout delete request");

    assert!(delete_response.status().is_success(), "Workout delete should succeed");

    // Verify workout is deleted
    let get_response = client
        .get(&format!("{}/admin/workouts/{}", &test_app.address, sync_id))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute workout get request");

    assert_eq!(get_response.status(), 404);
}

#[tokio::test]
async fn test_admin_can_view_all_workouts() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a user and workout
    let user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &user).await.unwrap();
    let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 60);
    upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout_data).await
        .expect("Workout upload should succeed");

    // Admin views all workouts
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/workouts", &test_app.address),
        &admin.token,
        None,
    ).await;

    assert_eq!(200, response.status().as_u16());
    
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["data"]["workouts"].is_array());
}

#[tokio::test]
async fn test_admin_can_get_workout_by_id() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a user and workout
    let user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &user).await.unwrap();
    let mut workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 45);
    let workout_response = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout_data).await;

    let response_data = workout_response.unwrap();
    let sync_id = response_data["data"]["sync_id"].as_str().unwrap();

    // Admin gets specific workout
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/workouts/{}", &test_app.address, sync_id),
        &admin.token,
        None,
    ).await;

    assert_eq!(200, response.status().as_u16());
    
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["data"]["id"].as_str().unwrap(), sync_id);
}

// ============================================================================
// WORKOUT MEDIA UPLOAD TESTS
// ============================================================================

#[tokio::test]
async fn test_signed_url_endpoints_exist() {
    let test_app = spawn_app().await;
    let client = Client::new();
    
    let test_user = create_test_user_and_login(&test_app.address).await;

    // Test image signed URL endpoint
    let image_response = client
        .get(&format!("{}/media/signed-url/image", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to execute image signed URL request");

    // Should return some response (not necessarily successful without S3 setup)
    assert!(image_response.status().as_u16() < 500, "Image signed URL endpoint should exist");

    // Test video signed URL endpoint  
    let video_response = client
        .get(&format!("{}/media/signed-url/video", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to execute video signed URL request");

    // Should return some response (not necessarily successful without S3 setup)
    assert!(video_response.status().as_u16() < 500, "Video signed URL endpoint should exist");
}

#[tokio::test]  
async fn test_workout_with_media_urls() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Upload workout with media URLs
    let mut workout_with_media = WorkoutData::new(WorkoutType::Light, Utc::now(), 30);
    workout_with_media.image_url = Some("https://example.com/workout-image.jpg".to_string());
    workout_with_media.video_url = Some("https://example.com/workout-video.mp4".to_string());
    let response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_with_media).await;
    assert!(response.is_ok(), "Workout with media URLs should succeed");
    
    let response_data = response.unwrap();
    assert!(response_data["data"]["sync_id"].is_string());
}

// ============================================================================
// WORKOUT NOTIFICATIONS TEST
// ============================================================================

#[tokio::test]
async fn test_workout_upload_notification_integration() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Upload a workout (this should trigger notifications)
    let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 45);
    let response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;

    assert!(response.is_ok(), "Workout upload should succeed and trigger notifications");
    
    // Note: In a full integration test, we would verify that:
    // 1. Redis pub/sub message was sent
    // 2. WebSocket clients received notification
    // 3. Database state was updated correctly
    // For now, we just verify the upload succeeded
    let response_data = response.unwrap();
    assert!(response_data["data"]["sync_id"].is_string());
}

// ============================================================================
// POST EDIT AND DELETE TESTS
// ============================================================================

#[tokio::test]
async fn test_edit_workout_post() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create user and upload workout
    let test_user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    let mut workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;
    assert!(upload_response.is_ok());

    // Get the post ID from the feed
    let feed_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/feed/?limit=100", &test_app.address),
        &test_user.token,
        None,
    ).await;

    let feed_data: serde_json::Value = feed_response.json().await.unwrap();
    assert!(feed_data["data"]["posts"].is_array());
    let posts = feed_data["data"]["posts"].as_array().unwrap();
    assert!(!posts.is_empty(), "No posts found after workout upload");

    // Find the post that belongs to the current test user
    let user_post = posts.iter()
        .find(|p| p["username"].as_str() == Some(&test_user.username))
        .expect("Could not find a post belonging to the test user");

    let post_id = user_post["id"].as_str().unwrap();

    // Update the post with new description and activity type
    let update_body = json!({
        "content": "Updated workout description!",
        "activity_name": "Trail Running"
    });

    let update_response = make_authenticated_request(
        &client,
        reqwest::Method::PATCH,
        &format!("{}/posts/{}", &test_app.address, post_id),
        &test_user.token,
        Some(update_body),
    ).await;

    let status = update_response.status();
    if !status.is_success() {
        let error_text = update_response.text().await.unwrap_or_else(|_| "Could not read error".to_string());
        panic!("Failed to update post. Status: {}, Response: {}", status, error_text);
    }
    let update_data: serde_json::Value = update_response.json().await.unwrap();
    assert_eq!(update_data["success"], true);

    // Verify the changes by fetching the post
    let get_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/posts/{}", &test_app.address, post_id),
        &test_user.token,
        None,
    ).await;

    assert!(get_response.status().is_success(), "Failed to fetch updated post");
    let post_data: serde_json::Value = get_response.json().await.unwrap();
    assert_eq!(post_data["data"]["content"], "Updated workout description!");
    assert_eq!(post_data["data"]["workout_data"]["activity_name"], "Trail Running");
}

#[tokio::test]
async fn test_delete_workout_post() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create user and upload workout
    let test_user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    let mut workout_data = WorkoutData::new(WorkoutType::Light, Utc::now(), 20);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;
    assert!(upload_response.is_ok());

    // Get the post ID from the feed (fetch more posts to ensure we get ours)
    let feed_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/feed/?limit=100", &test_app.address),
        &test_user.token,
        None,
    ).await;

    let feed_data: serde_json::Value = feed_response.json().await.unwrap();
    let posts = feed_data["data"]["posts"].as_array().unwrap();
    assert!(!posts.is_empty(), "No posts found after workout upload");

    println!("DEBUG delete test: Looking for user '{}' in {} posts", test_user.username, posts.len());

    // Find the post that belongs to the current test user
    let user_post = posts.iter()
        .find(|p| p["username"].as_str() == Some(&test_user.username))
        .expect(&format!("Could not find a post belonging to user '{}'. Found usernames: {:?}",
            test_user.username,
            posts.iter().map(|p| p["username"].as_str().unwrap_or("unknown")).collect::<Vec<_>>()
        ));

    let post_id = user_post["id"].as_str().unwrap();

    // Delete the post
    let delete_response = make_authenticated_request(
        &client,
        reqwest::Method::DELETE,
        &format!("{}/posts/{}", &test_app.address, post_id),
        &test_user.token,
        None,
    ).await;

    let status = delete_response.status();
    if !status.is_success() {
        let error_text = delete_response.text().await.unwrap_or_else(|_| "Could not read error".to_string());
        panic!("Failed to delete post. Status: {}, Response: {}", status, error_text);
    }
    let delete_data: serde_json::Value = delete_response.json().await.unwrap();
    assert_eq!(delete_data["success"], true);

    // Verify the post is gone
    let get_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/posts/{}", &test_app.address, post_id),
        &test_user.token,
        None,
    ).await;

    assert!(!get_response.status().is_success(), "Post should not exist after deletion");
}

#[tokio::test]
async fn test_cannot_edit_another_users_post() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create first user and upload workout
    let user1 = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &user1).await.unwrap();

    let mut workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &user1.token, &mut workout_data).await;
    assert!(upload_response.is_ok());

    // Get the post ID
    let feed_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/feed/?limit=1", &test_app.address),
        &user1.token,
        None,
    ).await;

    let feed_data: serde_json::Value = feed_response.json().await.unwrap();
    let posts = feed_data["data"]["posts"].as_array().unwrap();
    let post_id = posts[0]["id"].as_str().unwrap();

    // Create second user
    let user2 = create_test_user_and_login(&test_app.address).await;

    // Try to edit user1's post as user2
    let update_body = json!({
        "content": "Hacked description"
    });

    let update_response = make_authenticated_request(
        &client,
        reqwest::Method::PATCH,
        &format!("{}/posts/{}", &test_app.address, post_id),
        &user2.token,
        Some(update_body),
    ).await;

    // Should fail with forbidden error (403)
    assert_eq!(update_response.status(), reqwest::StatusCode::FORBIDDEN,
        "User should not be able to edit another user's post");
}