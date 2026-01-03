//! Notifications integration tests
//!
//! Tests for notification functionality:
//! - Workout reaction notifications
//! - Comment notifications
//! - Reply notifications
//! - Comment reaction notifications
//! - Mark as read functionality
//! - WebSocket broadcast
//! - No self-notifications

use reqwest::Client;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use futures_util::StreamExt;
use std::time::Duration;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};
use common::social_helpers::create_user_with_workout;

// Helper function to filter out system notifications (like player_pool events)
fn filter_notifications_by_type<'a>(
    notifications: &'a [serde_json::Value],
    notification_type: &str
) -> Vec<&'a serde_json::Value> {
    notifications.iter()
        .filter(|n| n["notification_type"] == notification_type)
        .collect()
}

#[tokio::test]
async fn test_notification_on_workout_reaction() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User2 reacts to user1's workout
    let reaction_data = json!({"reaction_type": "fire"});
    client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");

    // Check user1's notifications
    let response = client
        .get(&format!("{}/social/notifications", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get notifications");

    assert!(response.status().is_success());
    let result: serde_json::Value = response.json().await.expect("Failed to parse response");

    // Filter to only reaction notifications (ignore player_pool and other system notifications)
    let all_notifications = result["notifications"].as_array().unwrap();
    let reaction_notifications: Vec<_> = all_notifications.iter()
        .filter(|n| n["notification_type"] == "reaction" && n["entity_type"] == "workout")
        .collect();

    assert_eq!(reaction_notifications.len(), 1, "Should have exactly 1 workout reaction notification");
    assert_eq!(reaction_notifications[0]["notification_type"], "reaction");
    assert_eq!(reaction_notifications[0]["entity_type"], "workout");
    assert_eq!(reaction_notifications[0]["read"], false);

}

#[tokio::test]
async fn test_notification_on_workout_comment() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User2 comments on user1's workout
    let comment_data = json!({"content": "Great workout!", "parent_id": null});
    client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");

    // Check user1's notifications
    let response = client
        .get(&format!("{}/social/notifications", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get notifications");

    assert!(response.status().is_success());
    let result: serde_json::Value = response.json().await.expect("Failed to parse response");

    // Filter to only comment notifications (ignore player_pool and other system notifications)
    let all_notifications = result["notifications"].as_array().unwrap();

    // Debug: print all notifications to see what types we have
    if all_notifications.len() > 0 {
        eprintln!("\nDEBUG: Found {} total notifications:", all_notifications.len());
        for (i, n) in all_notifications.iter().enumerate() {
            eprintln!("  [{}]: type='{}', entity_type='{}'",
                i, n["notification_type"], n["entity_type"]);
        }
    }

    let comment_notifications: Vec<_> = all_notifications.iter()
        .filter(|n| n["notification_type"] == "comment" && n["entity_type"] == "post")
        .collect();

    assert_eq!(comment_notifications.len(), 1, "Should have exactly 1 workout comment notification");
    assert_eq!(comment_notifications[0]["notification_type"], "comment");
    assert_eq!(comment_notifications[0]["entity_type"], "post");
    assert_eq!(comment_notifications[0]["read"], false);

}

#[tokio::test]
async fn test_notification_on_comment_reply() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User1 adds a comment
    let comment_data = json!({"content": "Great workout!", "parent_id": null});
    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user1.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");

    let comment: serde_json::Value = response.json().await.expect("Failed to parse response");
    let parent_id = comment["id"].as_str().unwrap();

    // User2 replies to user1's comment
    let reply_data = json!({"content": "Thanks!", "parent_id": parent_id});
    client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&reply_data)
        .send()
        .await
        .expect("Failed to add reply");

    // Check user1's notifications
    let response = client
        .get(&format!("{}/social/notifications", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get notifications");

    assert!(response.status().is_success());
    let result: serde_json::Value = response.json().await.expect("Failed to parse response");

    // Filter to only reply notifications
    let all_notifications = result["notifications"].as_array().unwrap();
    let reply_notifications: Vec<_> = all_notifications.iter()
        .filter(|n| n["notification_type"] == "reply")
        .collect();

    assert_eq!(reply_notifications.len(), 1, "Should have exactly 1 reply notification");
    assert_eq!(reply_notifications[0]["notification_type"], "reply");
    assert_eq!(reply_notifications[0]["entity_type"], "comment");
    assert_eq!(reply_notifications[0]["read"], false);

}

#[tokio::test]
async fn test_no_notification_for_own_reaction() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // User reacts to their own workout
    let reaction_data = json!({"reaction_type": "fire"});
    client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");

    // Small delay to ensure notification processing completes
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check user's notifications (should be empty)
    let response = client
        .get(&format!("{}/social/notifications", test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get notifications");

    assert!(response.status().is_success());
    let result: serde_json::Value = response.json().await.expect("Failed to parse response");

    // Filter out system notifications (player_pool, etc) and only check reaction notifications
    let notifications = result["notifications"].as_array().unwrap();
    let reaction_notifications: Vec<_> = notifications.iter()
        .filter(|n| n["notification_type"] == "reaction" && n["entity_type"] == "workout")
        .collect();

    assert_eq!(reaction_notifications.len(), 0, "Should have 0 reaction notifications for own workout");

}

#[tokio::test]
async fn test_mark_notification_as_read() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User2 reacts to user1's workout
    let reaction_data = json!({"reaction_type": "fire"});
    client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");

    // Get notification ID
    let response = client
        .get(&format!("{}/social/notifications", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get notifications");

    let result: serde_json::Value = response.json().await.expect("Failed to parse response");

    // Get the reaction notification (filter out system notifications)
    let all_notifications = result["notifications"].as_array().unwrap();
    let reaction_notifs = filter_notifications_by_type(all_notifications, "reaction");
    assert!(!reaction_notifs.is_empty(), "Should have at least one reaction notification");
    let notification_id = reaction_notifs[0]["id"].as_str().unwrap();

    // Mark as read
    let response = client
        .put(&format!("{}/social/notifications/{}/read", test_app.address, notification_id))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to mark as read");

    assert!(response.status().is_success());

    // Verify it's marked as read
    let response = client
        .get(&format!("{}/social/notifications", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get notifications");

    let result: serde_json::Value = response.json().await.expect("Failed to parse response");

    // Filter to only workout reaction notifications (ignore system notifications)
    let all_notifications = result["notifications"].as_array().unwrap();
    let reaction_notifs = filter_notifications_by_type(all_notifications, "reaction");
    let workout_reactions: Vec<_> = reaction_notifs.iter()
        .filter(|n| n["entity_type"] == "workout")
        .collect();

    assert_eq!(workout_reactions.len(), 1, "Should have 1 workout reaction notification");
    assert_eq!(workout_reactions[0]["read"], true, "Workout reaction notification should be marked as read");

}

#[tokio::test]
async fn test_mark_all_notifications_as_read() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User2 reacts and comments
    let reaction_data = json!({"reaction_type": "fire"});
    client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");

    let comment_data = json!({"content": "Great workout!", "parent_id": null});
    client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");

    // Mark all as read
    let response = client
        .put(&format!("{}/social/notifications/mark-all-read", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to mark all as read");

    assert!(response.status().is_success());

    // Verify all are marked as read
    let response = client
        .get(&format!("{}/social/notifications", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get notifications");

    let result: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(result["unread_count"], 0);

    let notifications = result["notifications"].as_array().unwrap();
    for notification in notifications {
        assert_eq!(notification["read"], true);

    }

}

#[tokio::test]
async fn test_get_unread_notification_count() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User2 adds multiple reactions and comments
    let reaction_data = json!({"reaction_type": "fire"});
    client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");

    let comment_data = json!({"content": "Great workout!", "parent_id": null});
    client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");

    // Get notifications and count unread ones (excluding system notifications)
    let response = client
        .get(&format!("{}/social/notifications", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get notifications");

    assert!(response.status().is_success());
    let result: serde_json::Value = response.json().await.expect("Failed to parse response");

    let all_notifications = result["notifications"].as_array().unwrap();
    let relevant_unread = all_notifications.iter()
        .filter(|n| {
            let notif_type = n["notification_type"].as_str().unwrap_or("");
            let is_relevant = matches!(notif_type, "reaction" | "comment" | "reply" | "comment_reaction");
            let is_unread = n["read"] == false;
            is_relevant && is_unread
        })
        .count();

    assert_eq!(relevant_unread, 2, "Should have exactly 2 unread notifications (1 reaction + 1 comment)");

}

#[tokio::test]
async fn test_notification_websocket_broadcast() {
    let test_app = spawn_app().await;
    let client = Client::new();

    tokio::time::sleep(Duration::from_secs(1)).await;

    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User1 connects to WebSocket
    let ws_url = format!("{}/game-ws?token={}", test_app.address.replace("http", "ws"), user1.token);
    let request = ws_url.into_client_request().expect("Failed to create request");
    let (mut ws_stream, _) = connect_async(request).await.expect("Failed to connect");

    // Consume welcome message
    let _welcome_msg = ws_stream.next().await.expect("No welcome message").unwrap();

    // User2 reacts to user1's workout
    let reaction_data = json!({"reaction_type": "fire"});
    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");

    assert!(response.status().is_success());

    // Listen for WebSocket notification event
    let mut notification_received = false;
    for _ in 0..10 {
        if let Ok(Some(Ok(Message::Text(text)))) = tokio::time::timeout(Duration::from_millis(500), ws_stream.next()).await {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                if event["event_type"] == "notification_received" {
                    notification_received = true;
                    assert_eq!(event["notification_type"], "reaction");
                    break;
                }
            }
        }
    }

    assert!(notification_received, "WebSocket notification event should be received");

}

#[tokio::test]
async fn test_notification_not_sent_to_actor() {
    let test_app = spawn_app().await;
    let client = Client::new();

    tokio::time::sleep(Duration::from_secs(1)).await;

    let (_user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User2 connects to WebSocket
    let ws_url = format!("{}/game-ws?token={}", test_app.address.replace("http", "ws"), user2.token);
    let request = ws_url.into_client_request().expect("Failed to create request");
    let (mut ws_stream, _) = connect_async(request).await.expect("Failed to connect");

    // Consume welcome message
    let _welcome_msg = ws_stream.next().await.expect("No welcome message").unwrap();

    // User2 reacts to user1's workout
    let reaction_data = json!({"reaction_type": "fire"});
    let response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");

    assert!(response.status().is_success());

    // Listen for WebSocket events - user2 should NOT receive the notification
    let mut notification_received = false;
    for _ in 0..5 {
        if let Ok(Some(Ok(Message::Text(text)))) = tokio::time::timeout(Duration::from_millis(500), ws_stream.next()).await {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                if event["event_type"] == "notification_received" {
                    notification_received = true;
                    break;
                }
            }
        }
    }

    assert!(!notification_received, "Actor should not receive notification for their own action");

}

#[tokio::test]
async fn test_notification_on_comment_reaction() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User1 adds a comment
    let comment_data = json!({"content": "Great workout!", "parent_id": null});
    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user1.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");

    let comment: serde_json::Value = response.json().await.expect("Failed to parse response");
    let comment_id = comment["id"].as_str().unwrap();

    // User2 reacts to user1's comment
    let reaction_data = json!({"reaction_type": "fire"});
    client
        .post(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add comment reaction");

    // Check user1's notifications
    let response = client
        .get(&format!("{}/social/notifications", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get notifications");

    assert!(response.status().is_success());
    let result: serde_json::Value = response.json().await.expect("Failed to parse response");

    // Filter to only comment reaction notifications
    let all_notifications = result["notifications"].as_array().unwrap();
    let comment_reaction_notifs = filter_notifications_by_type(all_notifications, "reaction");
    let comment_reactions_on_comments: Vec<_> = comment_reaction_notifs.iter()
        .filter(|n| n["entity_type"] == "comment")
        .collect();

    assert_eq!(comment_reactions_on_comments.len(), 1, "Should have exactly 1 comment reaction notification");
    assert_eq!(comment_reactions_on_comments[0]["notification_type"], "reaction");
    assert_eq!(comment_reactions_on_comments[0]["entity_type"], "comment");
    assert_eq!(comment_reactions_on_comments[0]["read"], false);

}

#[tokio::test]
async fn test_no_notification_for_own_comment_reaction() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let (user, workout_id) = create_user_with_workout(&test_app.address).await;

    // User adds a comment
    let comment_data = json!({"content": "Great workout!", "parent_id": null});
    let response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");

    let comment: serde_json::Value = response.json().await.expect("Failed to parse response");
    let comment_id = comment["id"].as_str().unwrap();

    // User reacts to their own comment
    let reaction_data = json!({"reaction_type": "fire"});
    client
        .post(&format!("{}/social/comments/{}/reactions", test_app.address, comment_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add comment reaction");

    // Check user's notifications (should be empty)
    let response = client
        .get(&format!("{}/social/notifications", test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get notifications");

    assert!(response.status().is_success());
    let result: serde_json::Value = response.json().await.expect("Failed to parse response");

    // Filter out system notifications and only check comment reaction notifications
    let notifications = result["notifications"].as_array().unwrap();
    let comment_reaction_notifications: Vec<_> = notifications.iter()
        .filter(|n| n["notification_type"] == "comment_reaction")
        .collect();

    assert_eq!(comment_reaction_notifications.len(), 0, "Should have 0 comment reaction notifications for own comment");

}
