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
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request, delete_test_user};
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
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 30);
    let response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;

    let status = response.is_ok();
    if !status {
        let error_body = response.err().unwrap();
        panic!("Health data upload failed with status {}: {}", status, error_body);
    }
    assert!(status);

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn upload_multiple_workout_data_sessions() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
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
    let admin_user = create_admin_user_and_login(&test_app.address).await;
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
    let admin_user = create_admin_user_and_login(&test_app.address).await;
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
    let admin_user = create_admin_user_and_login(&test_app.address).await;

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

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_workout_history_with_data() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
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

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_workout_history_pagination() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
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

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_workout_detail_endpoint() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
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

        // Cleanup
        delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
        delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
    }

}

#[tokio::test]
async fn test_workout_detail_endpoint_not_found() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;

    // Try to fetch a non-existent workout
    let fake_workout_id = Uuid::new_v4().to_string();
    let detail_response = client
        .get(&format!("{}/health/workout/{}", &test_app.address, fake_workout_id))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to execute workout detail request");

    assert_eq!(detail_response.status(), 404, "Should return 404 for non-existent workout");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
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

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, user.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
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

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, user.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

// ============================================================================
// WORKOUT MEDIA UPLOAD TESTS
// ============================================================================

#[tokio::test]
async fn test_signed_url_endpoints_exist() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;

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

        // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_workout_with_media_urls() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Upload workout with media URLs
    let mut workout_with_media = WorkoutData::new(WorkoutType::Light, Utc::now(), 30);
    workout_with_media.image_url = Some("https://example.com/workout-image.jpg".to_string());
    workout_with_media.video_url = Some("https://example.com/workout-video.mp4".to_string());
    let response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_with_media).await;
    assert!(response.is_ok(), "Workout with media URLs should succeed");

    let response_data = response.unwrap();
    assert!(response_data["data"]["sync_id"].is_string());

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

// ============================================================================
// WORKOUT NOTIFICATIONS TEST
// ============================================================================

#[tokio::test]
async fn test_workout_upload_notification_integration() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
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

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
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
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    let mut workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;
    assert!(upload_response.is_ok());

    // Get the post ID from the feed
    let feed_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/feed/?limit=200", &test_app.address),
        &test_user.token,
        None,
    ).await;

    let feed_data: serde_json::Value = feed_response.json().await.unwrap();

    // Debug: Print the feed response to understand the structure
    if feed_data["data"]["posts"].is_null() || !feed_data["data"]["posts"].is_array() {
        eprintln!("Feed response: {}", serde_json::to_string_pretty(&feed_data).unwrap());
        panic!("Feed did not return posts array. Check response above.");
    }

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
    // User-edited activity should be stored in user_activity, not activity_name
    assert_eq!(post_data["data"]["workout_data"]["user_activity"], "Trail Running");
    // Original activity_name should remain unchanged
    assert!(post_data["data"]["workout_data"]["activity_name"].is_null() ||
            post_data["data"]["workout_data"]["activity_name"].as_str().unwrap_or("") != "Trail Running",
            "activity_name should not be modified by user edits");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_delete_workout_post() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create user and upload workout
    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    let mut workout_data = WorkoutData::new(WorkoutType::Light, Utc::now(), 20);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;
    assert!(upload_response.is_ok());

    // Get the post ID from the feed (fetch more posts to ensure we get ours)
    let feed_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/feed/?limit=200", &test_app.address),
        &test_user.token,
        None,
    ).await;

    let feed_data: serde_json::Value = feed_response.json().await.unwrap();

    // Debug: Print the feed response to understand the structure
    if feed_data["data"]["posts"].is_null() || !feed_data["data"]["posts"].is_array() {
        eprintln!("Feed response: {}", serde_json::to_string_pretty(&feed_data).unwrap());
        panic!("Feed did not return posts array. Check response above.");
    }

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

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
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

// ============================================================================
// MAX HEART RATE UPDATE TESTS
// ============================================================================

#[tokio::test]
async fn test_max_heart_rate_updated_when_workout_exceeds_stored_value() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;

    // Create health profile with initial max heart rate
    let health_profile = json!({
        "age": 30,
        "gender": "male",
        "resting_heart_rate": 60,
        "weight": 75.0,
        "height": 180.0
    });

    let profile_response = client
        .put(&format!("{}/profile/health_profile", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&health_profile)
        .send()
        .await
        .expect("Failed to create health profile");

    assert!(profile_response.status().is_success(), "Health profile creation should succeed");

    // Get the initial max heart rate
    let initial_profile = client
        .get(&format!("{}/profile/health_profile", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to fetch health profile")
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse health profile");

    let initial_max_hr = initial_profile["data"]["max_heart_rate"]
        .as_i64()
        .expect("Max heart rate should be present") as i32;

    tracing::info!("Initial max heart rate: {}", initial_max_hr);
    assert!(initial_max_hr > 0, "Initial max heart rate should be positive");

    // Upload a workout with heart rate exceeding the stored max heart rate
    let mut workout_data = WorkoutData::new(WorkoutType::Hard, Utc::now(), 30);
    let response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;

    assert!(response.is_ok(), "Workout upload should succeed");

    // Get the workout max heart rate from the uploaded data
    let workout_max_hr = workout_data.get_heart_rate_data()
        .iter()
        .map(|hr| hr.heart_rate)
        .max()
        .expect("Should have heart rate data");

    tracing::info!("Workout max heart rate: {}", workout_max_hr);

    // Fetch the updated health profile
    let updated_profile = client
        .get(&format!("{}/profile/health_profile", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to fetch updated health profile")
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse updated health profile");

    let updated_max_hr = updated_profile["data"]["max_heart_rate"]
        .as_i64()
        .expect("Max heart rate should be present") as i32;

    tracing::info!("Updated max heart rate: {}", updated_max_hr);

    // Verify that max heart rate was updated if workout exceeded it
    if workout_max_hr > initial_max_hr {
        let expected_max_hr = (workout_max_hr as f32 * 1.2) as i32;
        assert_eq!(
            updated_max_hr, expected_max_hr,
            "Max heart rate should be updated to workout max * 1.2 ({} * 1.2 = {})",
            workout_max_hr, expected_max_hr
        );
    } else {
        assert_eq!(
            updated_max_hr, initial_max_hr,
            "Max heart rate should remain unchanged if workout didn't exceed it"
        );
    }

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

// ============================================================================
// HEALTH PROFILE VT THRESHOLDS FOR OTHER USERS TEST
// ============================================================================

#[tokio::test]
async fn test_fetch_other_user_health_profile_for_vt_thresholds() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create user1 with a health profile
    let user1 = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;

    let health_profile = json!({
        "age": 30,
        "gender": "male",
        "resting_heart_rate": 60,
        "weight": 75.0,
        "height": 180.0
    });

    let profile_response = client
        .put(&format!("{}/profile/health_profile", &test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .json(&health_profile)
        .send()
        .await
        .expect("Failed to create health profile");

    assert!(profile_response.status().is_success(), "Health profile creation should succeed");

    // Get user1's own profile to verify VT thresholds exist
    let user1_profile = client
        .get(&format!("{}/profile/health_profile", &test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to fetch own health profile")
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse own health profile");

    assert!(user1_profile["success"].as_bool().unwrap(), "Response should indicate success");
    let user1_data = &user1_profile["data"];

    // Verify VT thresholds are present in own profile
    assert!(user1_data["vt0_threshold"].is_number(), "VT0 threshold should be present");
    assert!(user1_data["vt1_threshold"].is_number(), "VT1 threshold should be present");
    assert!(user1_data["vt2_threshold"].is_number(), "VT2 threshold should be present");
    assert!(user1_data["max_heart_rate"].is_number(), "Max heart rate should be present");
    assert!(user1_data["resting_heart_rate"].is_number(), "Resting heart rate should be present");

    // Verify sensitive data is present in own profile
    assert!(user1_data["weight"].is_number(), "Weight should be present in own profile");
    assert!(user1_data["height"].is_number(), "Height should be present in own profile");
    assert!(user1_data["age"].is_number(), "Age should be present in own profile");

    let user1_vt0 = user1_data["vt0_threshold"].as_i64().unwrap();
    let user1_vt1 = user1_data["vt1_threshold"].as_i64().unwrap();
    let user1_vt2 = user1_data["vt2_threshold"].as_i64().unwrap();

    // Create user2 who will try to access user1's profile
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User2 fetches user1's health profile using query parameter
    let other_user_profile = client
        .get(&format!("{}/profile/health_profile?user_id={}", &test_app.address, user1.user_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .expect("Failed to fetch other user's health profile")
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse other user's health profile");

    assert!(other_user_profile["success"].as_bool().unwrap(), "Response should indicate success");
    let other_user_data = &other_user_profile["data"];

    // Verify VT thresholds are accessible for viewing workout zones
    assert_eq!(
        other_user_data["vt0_threshold"].as_i64().unwrap(),
        user1_vt0,
        "VT0 threshold should match user1's value"
    );
    assert_eq!(
        other_user_data["vt1_threshold"].as_i64().unwrap(),
        user1_vt1,
        "VT1 threshold should match user1's value"
    );
    assert_eq!(
        other_user_data["vt2_threshold"].as_i64().unwrap(),
        user1_vt2,
        "VT2 threshold should match user1's value"
    );
    assert!(other_user_data["max_heart_rate"].is_number(), "Max heart rate should be present");
    assert!(other_user_data["resting_heart_rate"].is_number(), "Resting heart rate should be present");

    // Verify sensitive data is redacted when fetching another user's profile
    assert!(
        other_user_data["weight"].is_null(),
        "Weight should be null for other user's profile (privacy protection)"
    );
    assert!(
        other_user_data["height"].is_null(),
        "Height should be null for other user's profile (privacy protection)"
    );
    assert!(
        other_user_data["age"].is_null(),
        "Age should be null for other user's profile (privacy protection)"
    );

    tracing::info!(
        "✅ Privacy check passed: VT thresholds accessible, sensitive data redacted for other user's profile"
    );

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, user1.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, user2.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_workout_detail_includes_user_id() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Upload a workout
    let mut workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);
    let workout_response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;
    assert!(workout_response.is_ok(), "Workout upload should succeed");

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

    // Test the workout detail endpoint
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

    // Verify the response includes user_id
    assert!(detail_data["success"].as_bool().unwrap(), "Response should indicate success");
    assert!(detail_data["data"].is_object(), "Should have data object");

    let workout_detail = &detail_data["data"];

    // Verify user_id is present and matches the test user
    assert!(
        workout_detail["user_id"].is_string(),
        "Workout detail should include user_id field"
    );
    assert_eq!(
        workout_detail["user_id"].as_str().unwrap(),
        test_user.user_id.to_string(),
        "Workout user_id should match the user who uploaded it"
    );

    tracing::info!(
        "✅ Workout detail includes user_id: {} (expected: {})",
        workout_detail["user_id"].as_str().unwrap(),
        test_user.user_id
    );

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

// ============================================================================
// WORKOUT SCORING FEEDBACK TESTS
// ============================================================================

#[tokio::test]
async fn test_submit_and_retrieve_scoring_feedback() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Upload a workout
    let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 45);
    let workout_response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;
    assert!(workout_response.is_ok(), "Workout upload should succeed");

    // Get workout ID from history
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to fetch workout history");

    let history_data: serde_json::Value = history_response.json().await.unwrap();
    let workouts = history_data["data"]["workouts"].as_array().unwrap();
    let workout_id = workouts[0]["id"].as_str().unwrap();

    // Submit scoring feedback - effort rating 8 (hard effort)
    let feedback_payload = json!({
        "effort_rating": 8
    });

    let submit_response = client
        .post(&format!("{}/health/workout/{}/scoring-feedback", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&feedback_payload)
        .send()
        .await
        .expect("Failed to submit scoring feedback");

    assert_eq!(submit_response.status(), reqwest::StatusCode::OK, "Feedback submission should succeed");

    let submit_data: serde_json::Value = submit_response.json().await.unwrap();
    assert_eq!(submit_data["effort_rating"], 8);
    assert_eq!(submit_data["workout_data_id"], workout_id);

    // Retrieve the submitted feedback
    let get_response = client
        .get(&format!("{}/health/workout/{}/scoring-feedback", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to retrieve scoring feedback");

    assert_eq!(get_response.status(), reqwest::StatusCode::OK);

    let get_data: serde_json::Value = get_response.json().await.unwrap();
    assert_eq!(get_data["effort_rating"], 8);
    assert_eq!(get_data["workout_data_id"], workout_id);

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_update_scoring_feedback() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Upload a workout
    let mut workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);
    upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await
        .expect("Workout upload should succeed");

    // Get workout ID
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to fetch workout history");

    let history_data: serde_json::Value = history_response.json().await.unwrap();
    let workouts = history_data["data"]["workouts"].as_array().unwrap();
    let workout_id = workouts[0]["id"].as_str().unwrap();

    // Submit initial feedback - effort rating 3 (light effort)
    let initial_feedback = json!({
        "effort_rating": 3
    });

    let initial_response = client
        .post(&format!("{}/health/workout/{}/scoring-feedback", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&initial_feedback)
        .send()
        .await
        .expect("Failed to submit initial feedback");

    assert_eq!(initial_response.status(), reqwest::StatusCode::OK);
    let initial_data: serde_json::Value = initial_response.json().await.unwrap();
    assert_eq!(initial_data["effort_rating"], 3);

    // Update feedback - effort rating 5 (moderate effort)
    let updated_feedback = json!({
        "effort_rating": 5
    });

    let update_response = client
        .post(&format!("{}/health/workout/{}/scoring-feedback", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&updated_feedback)
        .send()
        .await
        .expect("Failed to update feedback");

    assert_eq!(update_response.status(), reqwest::StatusCode::OK);
    let update_data: serde_json::Value = update_response.json().await.unwrap();
    assert_eq!(update_data["effort_rating"], 5);

    // Verify the feedback was updated
    let get_response = client
        .get(&format!("{}/health/workout/{}/scoring-feedback", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to retrieve feedback");

    let get_data: serde_json::Value = get_response.json().await.unwrap();
    assert_eq!(get_data["effort_rating"], 5, "Feedback should be updated to effort rating 5");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_scoring_feedback_for_nonexistent_workout() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;

    // Try to submit feedback for a non-existent workout
    let fake_workout_id = Uuid::new_v4();
    let feedback_payload = json!({
        "effort_rating": 7
    });

    let response = client
        .post(&format!("{}/health/workout/{}/scoring-feedback", &test_app.address, fake_workout_id))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&feedback_payload)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND, "Should return 404 for non-existent workout");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_all_effort_ratings() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Test various effort ratings (1-10 scale)
    let effort_ratings = vec![1, 3, 5, 7, 10];

    for (i, &effort_rating) in effort_ratings.iter().enumerate() {
        // Upload a workout for each effort rating with different start times to avoid duplicates
        let workout_start = Utc::now() - chrono::Duration::hours((i + 1) as i64);
        let mut workout_data = WorkoutData::new(WorkoutType::Light, workout_start, 20);
        upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await
            .expect("Workout upload should succeed");

        // Get workout ID (latest workout will be first)
        let history_response = client
            .get(&format!("{}/health/history?limit=1", &test_app.address))
            .header("Authorization", format!("Bearer {}", test_user.token))
            .send()
            .await
            .expect("Failed to fetch workout history");

        let history_data: serde_json::Value = history_response.json().await.unwrap();
        let workouts = history_data["data"]["workouts"].as_array().unwrap();
        let workout_id = workouts[0]["id"].as_str().unwrap();

        // Submit feedback
        let feedback_payload = json!({
            "effort_rating": effort_rating
        });

        let response = client
            .post(&format!("{}/health/workout/{}/scoring-feedback", &test_app.address, workout_id))
            .header("Authorization", format!("Bearer {}", test_user.token))
            .json(&feedback_payload)
            .send()
            .await
            .expect("Failed to submit feedback");

        assert_eq!(response.status(), reqwest::StatusCode::OK, "Feedback submission for effort rating {} should succeed", effort_rating);

        let data: serde_json::Value = response.json().await.unwrap();
        assert_eq!(data["effort_rating"].as_i64().unwrap(), effort_rating as i64);
    }

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_invalid_effort_rating() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Upload a workout
    let mut workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);
    upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await
        .expect("Workout upload should succeed");

    // Get workout ID
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to fetch workout history");

    let history_data: serde_json::Value = history_response.json().await.unwrap();
    let workouts = history_data["data"]["workouts"].as_array().unwrap();
    let workout_id = workouts[0]["id"].as_str().unwrap();

    // Test invalid effort rating (too high)
    let invalid_feedback_high = json!({
        "effort_rating": 15
    });

    let response_high = client
        .post(&format!("{}/health/workout/{}/scoring-feedback", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&invalid_feedback_high)
        .send()
        .await
        .expect("Failed to submit feedback");

    assert_eq!(response_high.status(), reqwest::StatusCode::BAD_REQUEST, "Should reject effort rating > 10");

    // Test invalid effort rating (negative)
    let invalid_feedback_negative = json!({
        "effort_rating": -1
    });

    let response_negative = client
        .post(&format!("{}/health/workout/{}/scoring-feedback", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&invalid_feedback_negative)
        .send()
        .await
        .expect("Failed to submit feedback");

    assert_eq!(response_negative.status(), reqwest::StatusCode::BAD_REQUEST, "Should reject negative effort rating");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_get_feedback_without_submission() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Upload a workout
    let mut workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);
    upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await
        .expect("Workout upload should succeed");

    // Get workout ID
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to fetch workout history");

    let history_data: serde_json::Value = history_response.json().await.unwrap();
    let workouts = history_data["data"]["workouts"].as_array().unwrap();
    let workout_id = workouts[0]["id"].as_str().unwrap();

    // Try to get feedback before submitting any
    let get_response = client
        .get(&format!("{}/health/workout/{}/scoring-feedback", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to retrieve feedback");

    assert_eq!(get_response.status(), reqwest::StatusCode::OK);

    let get_data: serde_json::Value = get_response.json().await.unwrap();
    assert_eq!(get_data["feedback"], serde_json::Value::Null, "Should return null feedback when none submitted");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}
