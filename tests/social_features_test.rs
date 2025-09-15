//! Social features integration tests
//!
//! This test suite covers all social functionality including:
//! - Workout reactions (add, remove, get reactions)
//! - Workout comments (add, edit, delete, get comments)
//! - Comment threading and replies
//! - Permission checks for edit/delete operations
//! - Pagination for comments
//! - Error handling for invalid operations

use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use chrono::Utc;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request, TestApp, UserRegLoginResponse};
use common::workout_data_helpers::{WorkoutData, WorkoutType, upload_workout_data_for_user};

// Helper function to create user with health profile and upload a workout
async fn create_user_with_workout(app_address: &str) -> (UserRegLoginResponse, Uuid) {
    let client = reqwest::Client::new();
    let user = create_test_user_and_login(app_address).await;

    // Create health profile for stats calculation
    let health_profile_data = json!({
        "age": 25,
        "gender": "male",
        "resting_heart_rate": 60
    });

    let profile_response = client
        .put(&format!("{}/profile/health_profile", app_address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&health_profile_data)
        .send()
        .await
        .expect("Failed to create health profile");

    assert!(profile_response.status().is_success(), "Health profile creation should succeed");

    // Upload a workout
    let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 30);
    let workout_response = upload_workout_data_for_user(&client, app_address, &user.token, &mut workout_data).await;
    assert!(workout_response.is_ok(), "Workout upload should succeed");

    let workout_response_data = workout_response.unwrap();
    let workout_id = Uuid::parse_str(workout_response_data["data"]["sync_id"].as_str().unwrap()).unwrap();
    (user, workout_id)
}

// ============================================================================
// WORKOUT REACTIONS TESTS
// ============================================================================

#[tokio::test]
async fn test_add_reaction_success() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add a reaction
    let reaction_data = json!({
        "reaction_type": "fire"
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to send reaction request");

    assert!(response.status().is_success(), "Should successfully add reaction");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["reaction_type"], "fire");
    assert_eq!(response_body["user_id"], user.user_id.to_string());
    assert_eq!(response_body["workout_id"], workout_id.to_string());
}

#[tokio::test]
async fn test_add_invalid_reaction_type_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add an invalid reaction type
    let reaction_data = json!({
        "reaction_type": "invalid_reaction"
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to send reaction request");

    assert_eq!(response.status(), 400, "Should fail with invalid reaction type");
}

#[tokio::test]
async fn test_update_existing_reaction() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add initial reaction
    let initial_reaction = json!({"reaction_type": "like"});
    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&initial_reaction)
        .send()
        .await
        .expect("Failed to send initial reaction");

    assert!(response.status().is_success(), "Initial reaction should succeed");

    // Update to different reaction type
    let updated_reaction = json!({"reaction_type": "fire"});
    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&updated_reaction)
        .send()
        .await
        .expect("Failed to send updated reaction");

    assert!(response.status().is_success(), "Updated reaction should succeed");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["reaction_type"], "fire", "Reaction should be updated");
}

#[tokio::test]
async fn test_remove_reaction_success() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // First add a reaction
    let reaction_data = json!({"reaction_type": "like"});
    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");

    assert!(response.status().is_success(), "Adding reaction should succeed");

    // Then remove it
    let response = client
        .delete(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to remove reaction");

    assert!(response.status().is_success(), "Removing reaction should succeed");
}

#[tokio::test]
async fn test_remove_nonexistent_reaction_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Try to remove a reaction that doesn't exist
    let response = client
        .delete(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to send remove request");

    assert_eq!(response.status(), 404, "Should fail when removing non-existent reaction");
}

#[tokio::test]
async fn test_get_workout_reactions() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // Add reactions from both users
    let reaction1 = json!({"reaction_type": "fire"});
    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user1.token))
        .json(&reaction1)
        .send()
        .await
        .expect("Failed to add first reaction");
    assert!(response.status().is_success());

    let reaction2 = json!({"reaction_type": "muscle"});
    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&reaction2)
        .send()
        .await
        .expect("Failed to add second reaction");
    assert!(response.status().is_success());

    // Get reactions summary
    let response = client
        .get(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get reactions");

    assert!(response.status().is_success());

    let reactions: serde_json::Value = response.json().await.expect("Failed to parse reactions");
    assert!(reactions.is_array(), "Should return array of reaction summaries");

    let reactions_array = reactions.as_array().unwrap();
    assert_eq!(reactions_array.len(), 2, "Should have 2 different reaction types");
}

#[tokio::test]
async fn test_get_reaction_users() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add a reaction
    let reaction_data = json!({"reaction_type": "fire"});
    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");
    assert!(response.status().is_success());

    // Get users who reacted
    let response = client
        .get(&format!("{}/social/workouts/{}/reactions/users", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get reaction users");

    assert!(response.status().is_success());

    let users: serde_json::Value = response.json().await.expect("Failed to parse users");
    assert!(users.is_array(), "Should return array of users");

    let users_array = users.as_array().unwrap();
    assert_eq!(users_array.len(), 1, "Should have 1 user who reacted");
    assert_eq!(users_array[0]["reaction_type"], "fire");
    assert_eq!(users_array[0]["username"], user.username);
}

// ============================================================================
// WORKOUT COMMENTS TESTS
// ============================================================================

#[tokio::test]
async fn test_add_comment_success() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add a comment
    let comment_data = json!({
        "content": "Great workout! Really inspiring.",
        "parent_id": null
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to send comment request");

    assert!(response.status().is_success(), "Should successfully add comment");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["content"], "Great workout! Really inspiring.");
    assert_eq!(response_body["user_id"], user.user_id.to_string());
    assert_eq!(response_body["workout_id"], workout_id.to_string());
    assert_eq!(response_body["is_edited"], false);
}

#[tokio::test]
async fn test_add_empty_comment_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Try to add an empty comment
    let comment_data = json!({
        "content": "",
        "parent_id": null
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to send comment request");

    assert_eq!(response.status(), 400, "Should fail with empty comment");
}

#[tokio::test]
async fn test_add_too_long_comment_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Try to add a comment that's too long (> 1000 characters)
    let long_content = "a".repeat(1001);
    let comment_data = json!({
        "content": long_content,
        "parent_id": null
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to send comment request");

    assert_eq!(response.status(), 400, "Should fail with too long comment");
}

#[tokio::test]
async fn test_edit_comment_success() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // First add a comment
    let comment_data = json!({
        "content": "Original comment",
        "parent_id": null
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");

    assert!(response.status().is_success());
    let comment: serde_json::Value = response.json().await.expect("Failed to parse comment");
    let comment_id = comment["id"].as_str().unwrap();

    // Then edit it
    let edit_data = json!({
        "content": "Edited comment"
    });

    let response = client
        .put(&format!("{}/social/comments/{}", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&edit_data)
        .send()
        .await
        .expect("Failed to edit comment");

    assert!(response.status().is_success(), "Should successfully edit comment");

    let edited_comment: serde_json::Value = response.json().await.expect("Failed to parse edited comment");
    assert_eq!(edited_comment["content"], "Edited comment");
    assert_eq!(edited_comment["is_edited"], true);
}

#[tokio::test]
async fn test_edit_other_users_comment_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User 1 adds a comment
    let comment_data = json!({
        "content": "User 1's comment",
        "parent_id": null
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user1.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");

    assert!(response.status().is_success());
    let comment: serde_json::Value = response.json().await.expect("Failed to parse comment");
    let comment_id = comment["id"].as_str().unwrap();

    // User 2 tries to edit User 1's comment
    let edit_data = json!({
        "content": "User 2 trying to edit"
    });

    let response = client
        .put(&format!("{}/social/comments/{}", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&edit_data)
        .send()
        .await
        .expect("Failed to send edit request");

    assert_eq!(response.status(), 404, "Should fail when editing other user's comment");
}

#[tokio::test]
async fn test_delete_comment_success() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // First add a comment
    let comment_data = json!({
        "content": "Comment to be deleted",
        "parent_id": null
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");

    assert!(response.status().is_success());
    let comment: serde_json::Value = response.json().await.expect("Failed to parse comment");
    let comment_id = comment["id"].as_str().unwrap();

    // Then delete it
    let response = client
        .delete(&format!("{}/social/comments/{}", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to delete comment");

    assert!(response.status().is_success(), "Should successfully delete comment");
}

#[tokio::test]
async fn test_get_workout_comments() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add multiple comments
    for i in 1..=3 {
        let comment_data = json!({
            "content": format!("Comment number {}", i),
            "parent_id": null
        });

        let response = client
            .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
            .header("Authorization", format!("Bearer {}", user.token))
            .json(&comment_data)
            .send()
            .await
            .expect("Failed to add comment");

        assert!(response.status().is_success());
    }

    // Get comments
    let response = client
        .get(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get comments");

    assert!(response.status().is_success());

    let comments_response: serde_json::Value = response.json().await.expect("Failed to parse comments");
    assert_eq!(comments_response["total_count"], 3);
    assert_eq!(comments_response["comments"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_comment_threading() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User 1 adds a parent comment
    let parent_comment = json!({
        "content": "This is the parent comment",
        "parent_id": null
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user1.token))
        .json(&parent_comment)
        .send()
        .await
        .expect("Failed to add parent comment");

    assert!(response.status().is_success());
    let parent: serde_json::Value = response.json().await.expect("Failed to parse parent comment");
    let parent_id = parent["id"].as_str().unwrap();

    // User 2 replies to the parent comment
    let reply_comment = json!({
        "content": "This is a reply to the parent",
        "parent_id": parent_id
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&reply_comment)
        .send()
        .await
        .expect("Failed to add reply comment");

    assert!(response.status().is_success());

    // Get comments and verify threading
    let response = client
        .get(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get comments");

    assert!(response.status().is_success());

    let comments_response: serde_json::Value = response.json().await.expect("Failed to parse comments");
    let comments = comments_response["comments"].as_array().unwrap();

    assert_eq!(comments.len(), 1, "Should have 1 top-level comment");
    assert_eq!(comments[0]["content"], "This is the parent comment");
    assert_eq!(comments[0]["replies"].as_array().unwrap().len(), 1, "Should have 1 reply");
    assert_eq!(comments[0]["replies"][0]["content"], "This is a reply to the parent");
}

#[tokio::test]
async fn test_comments_pagination() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add 5 comments
    for i in 1..=5 {
        let comment_data = json!({
            "content": format!("Comment {}", i),
            "parent_id": null
        });

        let response = client
            .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
            .header("Authorization", format!("Bearer {}", user.token))
            .json(&comment_data)
            .send()
            .await
            .expect("Failed to add comment");

        assert!(response.status().is_success());
    }

    // Get first page with 3 comments per page
    let response = client
        .get(&format!("{}/social/workouts/{}/comments?page=1&per_page=3", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get comments");

    assert!(response.status().is_success());

    let comments_response: serde_json::Value = response.json().await.expect("Failed to parse comments");
    assert_eq!(comments_response["total_count"], 5);
    assert_eq!(comments_response["comments"].as_array().unwrap().len(), 3);
    assert_eq!(comments_response["page"], 1);
    assert_eq!(comments_response["per_page"], 3);

    // Get second page
    let response = client
        .get(&format!("{}/social/workouts/{}/comments?page=2&per_page=3", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get comments page 2");

    assert!(response.status().is_success());

    let comments_response: serde_json::Value = response.json().await.expect("Failed to parse comments page 2");
    assert_eq!(comments_response["total_count"], 5);
    assert_eq!(comments_response["comments"].as_array().unwrap().len(), 2); // Remaining 2 comments
    assert_eq!(comments_response["page"], 2);
}

// ============================================================================
// AUTHENTICATION AND AUTHORIZATION TESTS
// ============================================================================

#[tokio::test]
async fn test_reactions_require_authentication() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (_, workout_id) = create_user_with_workout(&test_app.address).await;

    // Try to add reaction without token
    let reaction_data = json!({"reaction_type": "fire"});
    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 401, "Should require authentication");
}

#[tokio::test]
async fn test_comments_require_authentication() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (_, workout_id) = create_user_with_workout(&test_app.address).await;

    // Try to add comment without token
    let comment_data = json!({
        "content": "Unauthorized comment",
        "parent_id": null
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 401, "Should require authentication");
}

#[tokio::test]
async fn test_get_reactions_requires_authentication() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add a reaction first
    let reaction_data = json!({"reaction_type": "fire"});
    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");
    assert!(response.status().is_success());

    // Try to get reactions without authentication - should fail
    let response = client
        .get(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .send()
        .await
        .expect("Failed to send get reactions request");

    assert_eq!(response.status(), 401, "Getting reactions should require authentication");

    // Get reactions with authentication - should work
    let response = client
        .get(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get reactions with auth");

    assert!(response.status().is_success(), "Getting reactions should work with authentication");
}

#[tokio::test]
async fn test_get_comments_requires_authentication() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add a comment first
    let comment_data = json!({
        "content": "Private comment",
        "parent_id": null
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");
    assert!(response.status().is_success());

    // Try to get comments without authentication - should fail
    let response = client
        .get(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .send()
        .await
        .expect("Failed to send get comments request");

    assert_eq!(response.status(), 401, "Getting comments should require authentication");

    // Get comments with authentication - should work
    let response = client
        .get(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get comments with auth");

    assert!(response.status().is_success(), "Getting comments should work with authentication");
}