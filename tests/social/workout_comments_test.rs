//! Workout comments integration tests
//!
//! Tests for workout comment functionality:
//! - Adding comments
//! - Editing comments
//! - Deleting comments
//! - Comment threading and replies
//! - Pagination
//! - WebSocket events
//! - Authentication checks

use reqwest::Client;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use futures_util::StreamExt;
use std::time::Duration;

use crate::common::utils::{spawn_app, create_test_user_and_login};
use crate::common::social_helpers::create_user_with_workout;

#[tokio::test]
async fn test_add_comment_success() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    let comment_data = json!({
        "content": "Great workout!",
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
    let comment: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(comment["content"], "Great workout!");
    assert_eq!(comment["user_id"], user.user_id.to_string());
}

#[tokio::test]
async fn test_add_empty_comment_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

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
        .expect("Failed to send request");

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_add_too_long_comment_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

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
        .expect("Failed to send request");

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_edit_comment_success() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add a comment
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

    let comment: serde_json::Value = response.json().await.expect("Failed to parse response");
    let comment_id = comment["id"].as_str().unwrap();

    // Edit the comment
    let edit_data = json!({"content": "Edited comment"});
    let response = client
        .put(&format!("{}/social/comments/{}", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&edit_data)
        .send()
        .await
        .expect("Failed to edit comment");

    assert!(response.status().is_success());
    let edited_comment: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(edited_comment["content"], "Edited comment");
    assert_eq!(edited_comment["is_edited"], true);
}

#[tokio::test]
async fn test_edit_other_users_comment_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User1 adds a comment
    let comment_data = json!({
        "content": "User1's comment",
        "parent_id": null
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user1.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");

    let comment: serde_json::Value = response.json().await.expect("Failed to parse response");
    let comment_id = comment["id"].as_str().unwrap();

    // User2 tries to edit user1's comment
    let edit_data = json!({"content": "Hacked!"});
    let response = client
        .put(&format!("{}/social/comments/{}", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&edit_data)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_delete_comment_success() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add a comment
    let comment_data = json!({
        "content": "To be deleted",
        "parent_id": null
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");

    let comment: serde_json::Value = response.json().await.expect("Failed to parse response");
    let comment_id = comment["id"].as_str().unwrap();

    // Delete the comment
    let response = client
        .delete(&format!("{}/social/comments/{}", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to delete comment");

    assert!(response.status().is_success());
}

#[tokio::test]
async fn test_get_workout_comments() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add a comment
    let comment_data = json!({
        "content": "Test comment",
        "parent_id": null
    });

    client.post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");

    // Get comments
    let response = client
        .get(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get comments");

    assert!(response.status().is_success());
    let result: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(result["total_count"], 1);
    assert_eq!(result["comments"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_comment_threading() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add parent comment
    let parent_comment_data = json!({
        "content": "Parent comment",
        "parent_id": null
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&parent_comment_data)
        .send()
        .await
        .expect("Failed to add parent comment");

    let parent_comment: serde_json::Value = response.json().await.expect("Failed to parse response");
    let parent_id = parent_comment["id"].as_str().unwrap();

    // Add reply
    let reply_data = json!({
        "content": "Reply to parent",
        "parent_id": parent_id
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reply_data)
        .send()
        .await
        .expect("Failed to add reply");

    assert!(response.status().is_success());
    let reply: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(reply["parent_id"], parent_id);
}

#[tokio::test]
async fn test_comments_pagination() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add 25 comments
    for i in 0..25 {
        let comment_data = json!({
            "content": format!("Comment {}", i),
            "parent_id": null
        });

        client.post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
            .header("Authorization", format!("Bearer {}", user.token))
            .json(&comment_data)
            .send()
            .await
            .expect("Failed to add comment");
    }

    // Get first page
    let response = client
        .get(&format!("{}/social/workouts/{}/comments?page=1&per_page=10", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get comments");

    let result: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(result["total_count"], 25);
    assert_eq!(result["comments"].as_array().unwrap().len(), 10);
    assert_eq!(result["page"], 1);
}

#[tokio::test]
async fn test_comments_require_authentication() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (_, workout_id) = create_user_with_workout(&test_app.address).await;

    let comment_data = json!({
        "content": "Test comment",
        "parent_id": null
    });

    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_get_comments_requires_authentication() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (_, workout_id) = create_user_with_workout(&test_app.address).await;

    let response = client
        .get(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_websocket_comment_events_broadcast() {
    let test_app = spawn_app().await;
    let client = Client::new();

    tokio::time::sleep(Duration::from_secs(1)).await;

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Connect to WebSocket
    let ws_url = format!("{}/game-ws?token={}", test_app.address.replace("http", "ws"), user.token);
    let request = ws_url.into_client_request().expect("Failed to create request");
    let (mut ws_stream, _) = connect_async(request).await.expect("Failed to connect");

    // Consume welcome message
    let _welcome_msg = ws_stream.next().await.expect("No welcome message").unwrap();

    // Add a comment
    let comment_data = json!({
        "content": "Test comment",
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

    // Listen for WebSocket event
    let mut comment_event_received = false;
    for _ in 0..10 {
        if let Ok(Some(Ok(Message::Text(text)))) = tokio::time::timeout(Duration::from_millis(500), ws_stream.next()).await {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                if event["event_type"] == "workout_comment_added" {
                    comment_event_received = true;
                    break;
                }
            }
        }
    }

    assert!(comment_event_received, "WebSocket comment event should be received");
}
