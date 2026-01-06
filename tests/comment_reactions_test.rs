//! Comment reactions integration tests
//!
//! Tests for comment reaction functionality:
//! - Adding reactions to comments
//! - Removing reactions
//! - Getting reaction summaries
//! - Comment reactions on replies
//! - WebSocket events
//! - Multiple users scenarios
//! - Authentication checks

use reqwest::Client;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use futures_util::{StreamExt, SinkExt};
use std::time::Duration;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};
use common::social_helpers::create_user_with_workout;


#[tokio::test]
async fn test_add_comment_reaction_success() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // First, create a comment
    let comment_data = json!({
        "content": "Test comment for reaction"
    });

    let comment_response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to create comment");

    assert!(comment_response.status().is_success(), "Comment creation should succeed");
    let comment_response_body: serde_json::Value = comment_response.json().await.expect("Failed to parse comment response");
    let comment_id = comment_response_body["id"].as_str().unwrap();

    // Add a reaction to the comment
    let reaction_data = json!({
        "reaction_type": "fire"
    });

    let response = client
        .post(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to send comment reaction request");

    assert!(response.status().is_success(), "Should successfully add comment reaction");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["reaction_type"], "fire");
    assert_eq!(response_body["user_id"], user.user_id.to_string());
    assert_eq!(response_body["comment_id"], comment_id);

}

#[tokio::test]
async fn test_add_invalid_comment_reaction_type_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // First, create a comment
    let comment_data = json!({
        "content": "Test comment for invalid reaction"
    });

    let comment_response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to create comment");

    let comment_response_body: serde_json::Value = comment_response.json().await.expect("Failed to parse comment response");
    let comment_id = comment_response_body["id"].as_str().unwrap();

    // Try to add an invalid reaction type
    let reaction_data = json!({
        "reaction_type": "invalid_reaction"
    });

    let response = client
        .post(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to send comment reaction request");

    assert_eq!(response.status(), 400, "Should fail with invalid reaction type");

}

#[tokio::test]
async fn test_remove_comment_reaction_success() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // First, create a comment
    let comment_data = json!({
        "content": "Test comment for reaction removal"
    });

    let comment_response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to create comment");

    let comment_response_body: serde_json::Value = comment_response.json().await.expect("Failed to parse comment response");
    let comment_id = comment_response_body["id"].as_str().unwrap();

    // Add a reaction first
    let reaction_data = json!({
        "reaction_type": "fire"
    });

    let add_response = client
        .post(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add comment reaction");

    assert!(add_response.status().is_success(), "Adding reaction should succeed");

    // Remove the reaction
    let response = client
        .delete(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to remove comment reaction");

    assert!(response.status().is_success(), "Should successfully remove comment reaction");
}

#[tokio::test]
async fn test_remove_nonexistent_comment_reaction_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // First, create a comment
    let comment_data = json!({
        "content": "Test comment for nonexistent reaction removal"
    });

    let comment_response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to create comment");

    let comment_response_body: serde_json::Value = comment_response.json().await.expect("Failed to parse comment response");
    let comment_id = comment_response_body["id"].as_str().unwrap();

    // Try to remove a reaction that doesn't exist
    let response = client
        .delete(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to send remove reaction request");

    assert_eq!(response.status(), 404, "Should fail when trying to remove nonexistent reaction");
}

#[tokio::test]
async fn test_get_comment_reactions() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // First, create a comment
    let comment_data = json!({
        "content": "Test comment for getting reactions"
    });

    let comment_response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to create comment");

    let comment_response_body: serde_json::Value = comment_response.json().await.expect("Failed to parse comment response");
    let comment_id = comment_response_body["id"].as_str().unwrap();

    // Initially, there should be no reactions
    let response = client
        .get(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get comment reactions");

    assert!(response.status().is_success(), "Should successfully get comment reactions");
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["fire_count"], 0);
    assert_eq!(response_body["user_reacted"], false);

    // Add a reaction
    let reaction_data = json!({
        "reaction_type": "fire"
    });

    let add_response = client
        .post(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add comment reaction");

    assert!(add_response.status().is_success(), "Adding reaction should succeed");

    // Now get reactions again
    let response = client
        .get(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get comment reactions");

    assert!(response.status().is_success(), "Should successfully get comment reactions");
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["fire_count"], 1);
    assert_eq!(response_body["user_reacted"], true);
}

#[tokio::test]
async fn test_get_comment_reaction_users() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // First, create a comment
    let comment_data = json!({
        "content": "Test comment for getting reaction users"
    });

    let comment_response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to create comment");

    let comment_response_body: serde_json::Value = comment_response.json().await.expect("Failed to parse comment response");
    let comment_id = comment_response_body["id"].as_str().unwrap();

    // Add a reaction
    let reaction_data = json!({
        "reaction_type": "fire"
    });

    let add_response = client
        .post(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add comment reaction");

    assert!(add_response.status().is_success(), "Adding reaction should succeed");

    // Get users who reacted
    let response = client
        .get(&format!("{}/social/comments/{}/reactions/users", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get comment reaction users");

    assert!(response.status().is_success(), "Should successfully get comment reaction users");
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(response_body.is_array(), "Response should be an array");
    assert_eq!(response_body.as_array().unwrap().len(), 1, "Should have one user who reacted");
    
    let user_reaction = &response_body[0];
    assert_eq!(user_reaction["user_id"], user.user_id.to_string());
    assert_eq!(user_reaction["username"], user.username);
    assert_eq!(user_reaction["reaction_type"], "fire");
}

#[tokio::test]
async fn test_comment_reactions_in_workout_comments_response() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // First, create a comment
    let comment_data = json!({
        "content": "Test comment with reactions"
    });

    let comment_response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to create comment");

    let comment_response_body: serde_json::Value = comment_response.json().await.expect("Failed to parse comment response");
    let comment_id = comment_response_body["id"].as_str().unwrap();

    // Add a reaction to the comment
    let reaction_data = json!({
        "reaction_type": "fire"
    });

    let add_response = client
        .post(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add comment reaction");

    assert!(add_response.status().is_success(), "Adding reaction should succeed");

    // Get workout comments and verify reaction data is included
    let response = client
        .get(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get workout comments");

    assert!(response.status().is_success(), "Should successfully get workout comments");
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    let comments = response_body["comments"].as_array().unwrap();
    assert_eq!(comments.len(), 1, "Should have one comment");
    
    let comment = &comments[0];
    assert_eq!(comment["fire_count"], 1, "Comment should have 1 fire reaction");
    assert_eq!(comment["user_reacted"], true, "User should have reacted to the comment");
}

#[tokio::test]
async fn test_comment_reactions_on_replies() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // First, create a parent comment
    let parent_comment_data = json!({
        "content": "Parent comment"
    });

    let parent_comment_response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&parent_comment_data)
        .send()
        .await
        .expect("Failed to create parent comment");

    let parent_comment_response_body: serde_json::Value = parent_comment_response.json().await.expect("Failed to parse parent comment response");
    let parent_comment_id = parent_comment_response_body["id"].as_str().unwrap();

    // Create a reply
    let reply_data = json!({
        "content": "Reply to parent comment",
        "parent_id": parent_comment_id
    });

    let reply_response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reply_data)
        .send()
        .await
        .expect("Failed to create reply");

    let reply_response_body: serde_json::Value = reply_response.json().await.expect("Failed to parse reply response");
    let reply_id = reply_response_body["id"].as_str().unwrap();

    // Add a reaction to the reply
    let reaction_data = json!({
        "reaction_type": "fire"
    });

    let add_response = client
        .post(&format!("{}/social/comments/{}/reactions", test_app.address, reply_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction to reply");

    assert!(add_response.status().is_success(), "Adding reaction to reply should succeed");

    // Get workout comments and verify reply reaction data is included
    let response = client
        .get(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get workout comments");

    assert!(response.status().is_success(), "Should successfully get workout comments");
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    let comments = response_body["comments"].as_array().unwrap();
    assert_eq!(comments.len(), 1, "Should have one parent comment");
    
    let parent_comment = &comments[0];
    let replies = parent_comment["replies"].as_array().unwrap();
    assert_eq!(replies.len(), 1, "Should have one reply");
    
    let reply = &replies[0];
    assert_eq!(reply["fire_count"], 1, "Reply should have 1 fire reaction");
    assert_eq!(reply["user_reacted"], true, "User should have reacted to the reply");
}

#[tokio::test]
async fn test_comment_reaction_websocket_events() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Connect to WebSocket
    let ws_url = format!("{}/game-ws?token={}", test_app.address.replace("http", "ws"), user.token);
    let request = ws_url.into_client_request().expect("Failed to create request");

    let (mut ws_stream, _) = connect_async(request)
        .await
        .expect("Failed to connect to WebSocket");
    let (mut ws_sink, mut ws_stream) = ws_stream.split();

    // Subscribe to global events
    let subscribe_message = json!({
        "type": "subscribe",
        "channel": "game:events:global"
    });
    ws_sink.send(Message::Text(subscribe_message.to_string())).await.expect("Failed to send subscribe message");

    // First, create a comment
    let comment_data = json!({
        "content": "WebSocket test comment for reactions"
    });

    let comment_response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to create comment");

    let comment_response_body: serde_json::Value = comment_response.json().await.expect("Failed to parse comment response");
    let comment_id = comment_response_body["id"].as_str().unwrap();

    // Add a reaction to the comment
    let reaction_data = json!({
        "reaction_type": "fire"
    });

    let add_response = client
        .post(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add comment reaction");

    assert!(add_response.status().is_success(), "Adding reaction should succeed");

    // Listen for comment reaction added event
    let mut reaction_added_received = false;
    for _ in 0..50 {
        if let Ok(Some(Ok(Message::Text(text)))) = tokio::time::timeout(Duration::from_millis(500), ws_stream.next()).await {
            println!("Received WebSocket message: {}", text);
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                if event["event_type"] == "comment_reaction_added" &&
                   event["comment_id"] == comment_id &&
                   event["reaction_type"] == "fire" &&
                   event["username"] == user.username {
                    reaction_added_received = true;
                    break;
                }
            }
        }
    }

    assert!(reaction_added_received, "WebSocket comment reaction added event should be received");

    // Remove the reaction
    let remove_response = client
        .delete(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to remove comment reaction");

    assert!(remove_response.status().is_success(), "Removing reaction should succeed");

    // Listen for comment reaction removed event
    let mut reaction_removed_received = false;
    for _ in 0..50 {
        if let Ok(Some(Ok(Message::Text(text)))) = tokio::time::timeout(Duration::from_millis(500), ws_stream.next()).await {
            println!("Received WebSocket message: {}", text);
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                if event["event_type"] == "comment_reaction_removed" &&
                   event["comment_id"] == comment_id &&
                   event["username"] == user.username {
                    reaction_removed_received = true;
                    break;
                }
            }
        }
    }

    assert!(reaction_removed_received, "WebSocket comment reaction removed event should be received");
}

#[tokio::test]
async fn test_multiple_users_comment_reactions() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;

    // Create a second user
    let user2 = create_test_user_and_login(&test_app.address).await;

    // First, create a comment with user1
    let comment_data = json!({
        "content": "Comment for multiple user reactions"
    });

    let comment_response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user1.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to create comment");

    let comment_response_body: serde_json::Value = comment_response.json().await.expect("Failed to parse comment response");
    let comment_id = comment_response_body["id"].as_str().unwrap();

    // User1 adds a reaction
    let reaction_data = json!({
        "reaction_type": "fire"
    });

    let user1_reaction = client
        .post(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user1.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add user1 reaction");

    assert!(user1_reaction.status().is_success(), "User1 reaction should succeed");

    // User2 adds a reaction
    let user2_reaction = client
        .post(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add user2 reaction");

    assert!(user2_reaction.status().is_success(), "User2 reaction should succeed");

    // Get reactions and verify both users reacted
    let response = client
        .get(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get comment reactions");

    assert!(response.status().is_success(), "Should successfully get comment reactions");
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["fire_count"], 2, "Should have 2 fire reactions");
    assert_eq!(response_body["user_reacted"], true, "User1 should have reacted");

    // Get reaction users
    let users_response = client
        .get(&format!("{}/social/comments/{}/reactions/users", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get comment reaction users");

    assert!(users_response.status().is_success(), "Should successfully get comment reaction users");
    let users_body: serde_json::Value = users_response.json().await.expect("Failed to parse users response");
    assert_eq!(users_body.as_array().unwrap().len(), 2, "Should have 2 users who reacted");

}

#[tokio::test]
async fn test_comment_reaction_unauthorized_access() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // First, create a comment
    let comment_data = json!({
        "content": "Comment for unauthorized access test"
    });

    let comment_response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to create comment");

    let comment_response_body: serde_json::Value = comment_response.json().await.expect("Failed to parse comment response");
    let comment_id = comment_response_body["id"].as_str().unwrap();

    // Try to add reaction without authorization
    let reaction_data = json!({
        "reaction_type": "fire"
    });

    let response = client
        .post(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to send reaction request");

    assert_eq!(response.status(), 401, "Should fail without authorization");

    // Try to remove reaction without authorization
    let response = client
        .delete(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .send()
        .await
        .expect("Failed to send remove reaction request");

    assert_eq!(response.status(), 401, "Should fail without authorization");
}

// NEWSFEED TESTS
