//! Workout reactions integration tests
//!
//! Tests for workout reaction functionality:
//! - Adding reactions
//! - Updating reactions
//! - Removing reactions
//! - Getting reaction summaries and users
//! - WebSocket events
//! - Authentication checks

use reqwest::Client;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use futures_util::StreamExt;
use std::time::Duration;

use crate::common::utils::spawn_app;
use crate::common::social_helpers::create_user_with_workout;

#[tokio::test]
async fn test_add_reaction_success() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    let reaction_data = json!({"reaction_type": "fire"});
    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to send reaction request");

    assert!(response.status().is_success());
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["reaction_type"], "fire");
}

#[tokio::test]
async fn test_add_invalid_reaction_type_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    let reaction_data = json!({"reaction_type": "invalid_reaction"});
    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to send reaction request");

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_update_existing_reaction() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add initial reaction
    let reaction_data = json!({"reaction_type": "fire"});
    client.post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");

    // Update with different reaction type (should replace)
    let new_reaction_data = json!({"reaction_type": "fire"});
    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&new_reaction_data)
        .send()
        .await
        .expect("Failed to update reaction");

    assert!(response.status().is_success());
}

#[tokio::test]
async fn test_remove_reaction_success() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add a reaction first
    let reaction_data = json!({"reaction_type": "fire"});
    client.post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");

    // Remove the reaction
    let response = client
        .delete(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to remove reaction");

    assert!(response.status().is_success());
}

#[tokio::test]
async fn test_remove_nonexistent_reaction_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    let response = client
        .delete(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to send delete request");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_get_workout_reactions() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add a reaction
    let reaction_data = json!({"reaction_type": "fire"});
    client.post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");

    // Get reactions summary
    let response = client
        .get(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get reactions");

    assert!(response.status().is_success());
    let summary: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(summary["fire_count"], 1);
    assert_eq!(summary["user_reacted"], true);
}

#[tokio::test]
async fn test_get_reaction_users() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // Add a reaction
    let reaction_data = json!({"reaction_type": "fire"});
    client.post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");

    // Get users who reacted
    let response = client
        .get(&format!("{}/social/workouts/{}/reactions/users", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get reaction users");

    assert!(response.status().is_success());
    let users: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(users.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_reactions_require_authentication() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (_, workout_id) = create_user_with_workout(&test_app.address).await;

    let reaction_data = json!({"reaction_type": "fire"});
    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_get_reactions_requires_authentication() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (_, workout_id) = create_user_with_workout(&test_app.address).await;

    let response = client
        .get(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_websocket_reaction_events_broadcast() {
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

    // Listen for WebSocket event
    let mut reaction_event_received = false;
    for _ in 0..10 {
        if let Ok(Some(Ok(Message::Text(text)))) = tokio::time::timeout(Duration::from_millis(500), ws_stream.next()).await {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                if event["event_type"] == "workout_reaction_added" {
                    reaction_event_received = true;
                    break;
                }
            }
        }
    }

    assert!(reaction_event_received, "WebSocket reaction event should be received");
}
