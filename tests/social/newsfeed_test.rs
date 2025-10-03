//! Newsfeed integration tests
//!
//! Tests for newsfeed functionality:
//! - Getting workout feed
//! - Pagination with cursors
//! - Reactions and comments in feed
//! - Ordering (most recent first)
//! - Multiple workouts from different users
//! - Authentication checks

use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use chrono::Utc;
use url::form_urlencoded;

use crate::common::utils::{spawn_app, create_test_user_and_login};
use crate::common::social_helpers::create_user_with_workout;
use crate::common::workout_data_helpers::{WorkoutData, WorkoutType, upload_workout_data_for_user};

#[tokio::test]
async fn test_newsfeed_basic() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create two users with workouts
    let (user1, _workout1_id) = create_user_with_workout(&test_app.address).await;
    let (user2, _workout2_id) = create_user_with_workout(&test_app.address).await;

    // Get newsfeed for user1 with higher limit to ensure we get all workouts
    let response = client
        .get(&format!("{}/feed/?limit=50", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get newsfeed");

    let status = response.status();
    if !&status.is_success() {
        let error_body = response.text().await.expect("Failed to get error response");
        panic!("Feed request failed with status {}: {}", status, error_body);
    }
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");

    assert_eq!(response_body["success"], true);
    let workouts = response_body["data"]["workouts"].as_array().unwrap();

    // Should see at least both workouts (user1's and user2's) - other tests may add more
    assert!(workouts.len() >= 2, "Should have at least 2 workouts in feed, got {}", workouts.len());

    // Find the workouts from our specific users
    let user1_workouts: Vec<_> = workouts.iter()
        .filter(|w| w["user_id"] == user1.user_id.to_string())
        .collect();
    let user2_workouts: Vec<_> = workouts.iter()
        .filter(|w| w["user_id"] == user2.user_id.to_string())
        .collect();

    assert_eq!(user1_workouts.len(), 1, "Should have 1 workout from user1");
    assert_eq!(user2_workouts.len(), 1, "Should have 1 workout from user2");

    // Verify workouts have expected fields
    for workout in workouts {
        assert!(workout["id"].is_string());
        assert!(workout["user_id"].is_string());
        assert!(workout["username"].is_string());
        assert!(workout["workout_date"].is_string());
        assert!(workout["reaction_count"].is_number());
        assert!(workout["comment_count"].is_number());
        assert!(workout["user_has_reacted"].is_boolean());
        assert_eq!(workout["visibility"], "public");
    }
}

#[tokio::test]
async fn test_newsfeed_pagination() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create multiple workouts
    let (user, _) = create_user_with_workout(&test_app.address).await;

    // Upload more workouts
    for i in 0..3 {
        let mut workout_data = WorkoutData::new(
            WorkoutType::Moderate,
            Utc::now() - chrono::Duration::hours(i),
            30
        );
        let _ = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout_data).await;
    }

    // Get first page with limit
    let response = client
        .get(&format!("{}/feed/?limit=2", test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get first page");

    let status = response.status();
    if !&status.is_success() {
        let error_body = response.text().await.expect("Failed to get error response");
        panic!("Feed pagination request failed with status {}: {}", status, error_body);
    }
    let first_page: serde_json::Value = response.json().await.expect("Failed to parse response");

    let workouts = first_page["data"]["workouts"].as_array().unwrap();
    assert_eq!(workouts.len(), 2, "Should have 2 workouts in first page");

    let pagination = &first_page["data"]["pagination"];
    assert!(pagination["has_more"].as_bool().unwrap(), "Should have more pages");
    assert!(pagination["next_cursor"].is_string(), "Should have next cursor");

    // Get second page using cursor
    let cursor = pagination["next_cursor"].as_str().unwrap();
    let encoded_cursor = form_urlencoded::byte_serialize(cursor.as_bytes()).collect::<String>();
    let response = client
        .get(&format!("{}/feed/?limit=2&cursor={}", test_app.address, encoded_cursor))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get second page");

    let status = response.status();
    if !&status.is_success() {
        let error_body = response.text().await.expect("Failed to get error response");
        panic!("Feed second page request failed with status {}: {}", status, error_body);
    }
    let second_page: serde_json::Value = response.json().await.expect("Failed to parse response");

    let second_workouts = second_page["data"]["workouts"].as_array().unwrap();
    assert!(second_workouts.len() > 0, "Should have workouts in second page");

    // Verify no overlap between pages
    let first_ids: Vec<String> = workouts.iter()
        .map(|w| w["id"].as_str().unwrap().to_string())
        .collect();
    let second_ids: Vec<String> = second_workouts.iter()
        .map(|w| w["id"].as_str().unwrap().to_string())
        .collect();

    for id in &second_ids {
        assert!(!first_ids.contains(id), "Pages should not have overlapping workouts");
    }
}

#[tokio::test]
async fn test_newsfeed_with_reactions_and_comments() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create user with workout
    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User2 adds reaction to user1's workout
    let reaction_data = json!({"reaction_type": "fire"});
    let reaction_response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");

    assert!(reaction_response.status().is_success());

    // User2 adds comment to user1's workout
    let comment_data = json!({"content": "Great workout!", "parent_id": null});
    let comment_response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");

    assert!(comment_response.status().is_success());

    // Small delay to ensure data is committed
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Get newsfeed and verify counts
    let response = client
        .get(&format!("{}/feed/?limit=50", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get newsfeed");

    assert!(response.status().is_success());
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");

    let workouts = response_body["data"]["workouts"].as_array().unwrap();
    let user1_workout = workouts.iter()
        .find(|w| w["id"] == workout_id.to_string())
        .expect("Should find user1's workout in feed");

    assert_eq!(user1_workout["reaction_count"], 1, "Should have 1 reaction");
    assert_eq!(user1_workout["comment_count"], 1, "Should have 1 comment");
    assert_eq!(user1_workout["user_has_reacted"], false, "User1 has not reacted");
}

#[tokio::test]
async fn test_newsfeed_visibility_filter() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, _) = create_user_with_workout(&test_app.address).await;

    // Get newsfeed
    let response = client
        .get(&format!("{}/feed/?limit=50", test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get newsfeed");

    assert!(response.status().is_success());
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");

    let workouts = response_body["data"]["workouts"].as_array().unwrap();

    // All workouts in feed should be public
    for workout in workouts {
        assert_eq!(workout["visibility"], "public", "All feed workouts should be public");
    }
}

#[tokio::test]
async fn test_newsfeed_requires_authentication() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let response = client
        .get(&format!("{}/feed/", test_app.address))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 401, "Should require authentication");
}
