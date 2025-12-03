use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use std::time::Duration;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};

/// Helper to create a team via the league API (not admin API)
async fn create_team_via_league_api(app_address: &str, token: &str) -> String {
    let client = Client::new();

    let team_data = json!({
        "team_name": format!("Test Team {}", Uuid::new_v4()),
        "team_description": "A test team",
        "team_color": "#FF0000"
    });

    let response = client
        .post(&format!("{}/league/teams/register", app_address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&team_data)
        .send()
        .await
        .expect("Failed to create team");
    assert_eq!(response.status().as_u16(), 201);

    let team_body: serde_json::Value = response.json().await.unwrap();
    team_body["data"]["team_id"].as_str().unwrap().to_string()
}

/// Helper to add a member to a team using the league API
async fn add_team_member_via_api(app_address: &str, token: &str, team_id: &str, username: &str) {
    let client = Client::new();

    let add_member_data = json!({
        "member_request": [{
            "username": username,
            "role": "member"
        }]
    });

    let response = client
        .post(&format!("{}/league/teams/{}/members", app_address, team_id))
        .header("Authorization", format!("Bearer {}", token))
        .json(&add_member_data)
        .send()
        .await
        .expect("Failed to add member");
    assert_eq!(response.status().as_u16(), 201);
}

#[tokio::test]
async fn test_send_chat_message_success() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Send a chat message
    let message_data = json!({
        "message": "Hello team!"
    });

    let response = client
        .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&message_data)
        .send()
        .await
        .expect("Failed to send chat message");

    assert_eq!(response.status().as_u16(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap());
    assert_eq!(body["chat_message"]["message"].as_str().unwrap(), "Hello team!");
    assert_eq!(body["chat_message"]["username"].as_str().unwrap(), owner.username);
}

#[tokio::test]
async fn test_send_chat_message_not_team_member() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Create another user who is NOT on the team
    let non_member = create_test_user_and_login(&test_app.address).await;

    // Try to send a chat message
    let message_data = json!({
        "message": "I shouldn't be able to send this"
    });

    let response = client
        .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", non_member.token))
        .json(&message_data)
        .send()
        .await
        .expect("Failed to send chat message");

    assert_eq!(response.status().as_u16(), 403);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"].as_bool().unwrap(), false);
}

#[tokio::test]
async fn test_get_chat_history() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Send 5 messages
    for i in 1..=5 {
        let message_data = json!({
            "message": format!("Message {}", i)
        });

        let response = client
            .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
            .header("Authorization", format!("Bearer {}", owner.token))
            .json(&message_data)
            .send()
            .await
            .expect("Failed to send chat message");
        assert_eq!(response.status().as_u16(), 200);
    }

    // Get chat history
    let response = client
        .get(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("Failed to get chat history");

    assert_eq!(response.status().as_u16(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap());

    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 5);

    // Verify messages are in chronological order (oldest first)
    for i in 0..5 {
        assert_eq!(messages[i]["message"].as_str().unwrap(), format!("Message {}", i + 1));
    }
}

#[tokio::test]
async fn test_get_chat_history_with_pagination() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Send 10 messages
    for i in 1..=10 {
        let message_data = json!({
            "message": format!("Message {}", i)
        });

        client
            .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
            .header("Authorization", format!("Bearer {}", owner.token))
            .json(&message_data)
            .send()
            .await
            .expect("Failed to send chat message");
    }

    // Get first 5 messages
    let response = client
        .get(&format!("{}/league/teams/{}/chat?limit=5", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("Failed to get chat history");

    assert_eq!(response.status().as_u16(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 5);
    assert!(body["has_more"].as_bool().unwrap());

    // Messages are returned in chronological order (oldest first)
    // The first batch returns the 5 most recent messages: Message 6, 7, 8, 9, 10
    // To get the next (older) 5 messages, we need to use the ID of the oldest message in this batch
    // which is at index 0 (Message 6)
    let oldest_message_id_in_first_batch = messages[0]["id"].as_str().unwrap();

    // Get next 5 messages (older messages, before Message 6)
    let response = client
        .get(&format!("{}/league/teams/{}/chat?limit=5&before={}", test_app.address, team_id, oldest_message_id_in_first_batch))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("Failed to get chat history");

    assert_eq!(response.status().as_u16(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    let messages = body["messages"].as_array().unwrap();
    // Should get the remaining 5 older messages (Message 1, 2, 3, 4, 5)
    assert_eq!(messages.len(), 5);

    // Verify these are the older messages
    assert_eq!(messages[0]["message"].as_str().unwrap(), "Message 1");
    assert_eq!(messages[4]["message"].as_str().unwrap(), "Message 5");
}

#[tokio::test]
async fn test_edit_chat_message() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Send a message
    let message_data = json!({
        "message": "Original message"
    });

    let response = client
        .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&message_data)
        .send()
        .await
        .expect("Failed to send chat message");

    let body: serde_json::Value = response.json().await.unwrap();
    let message_id = body["chat_message"]["id"].as_str().unwrap();

    // Edit the message
    let edit_data = json!({
        "message": "Edited message"
    });

    let response = client
        .put(&format!("{}/league/teams/{}/chat/{}", test_app.address, team_id, message_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&edit_data)
        .send()
        .await
        .expect("Failed to edit chat message");

    assert_eq!(response.status().as_u16(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap());
    assert_eq!(body["chat_message"]["message"].as_str().unwrap(), "Edited message");
    assert!(body["chat_message"]["edited_at"].as_str().is_some());
}

#[tokio::test]
async fn test_edit_chat_message_not_owner() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Add a member
    let member = create_test_user_and_login(&test_app.address).await;
    add_team_member_via_api(&test_app.address, &owner.token, &team_id, &member.username).await;

    // Owner sends a message
    let message_data = json!({
        "message": "Owner's message"
    });

    let response = client
        .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&message_data)
        .send()
        .await
        .expect("Failed to send chat message");

    let body: serde_json::Value = response.json().await.unwrap();
    let message_id = body["chat_message"]["id"].as_str().unwrap();

    // Member tries to edit owner's message
    let edit_data = json!({
        "message": "Trying to edit someone else's message"
    });

    let response = client
        .put(&format!("{}/league/teams/{}/chat/{}", test_app.address, team_id, message_id))
        .header("Authorization", format!("Bearer {}", member.token))
        .json(&edit_data)
        .send()
        .await
        .expect("Failed to edit chat message");

    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn test_delete_own_message() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Send a message
    let message_data = json!({
        "message": "Message to delete"
    });

    let response = client
        .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&message_data)
        .send()
        .await
        .expect("Failed to send chat message");

    let body: serde_json::Value = response.json().await.unwrap();
    let message_id = body["chat_message"]["id"].as_str().unwrap();

    // Delete the message
    let response = client
        .delete(&format!("{}/league/teams/{}/chat/{}", test_app.address, team_id, message_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("Failed to delete chat message");

    assert_eq!(response.status().as_u16(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap());

    // Verify message is not in chat history
    let response = client
        .get(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("Failed to get chat history");

    let body: serde_json::Value = response.json().await.unwrap();
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 0);
}

#[tokio::test]
async fn test_admin_delete_any_message() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Add a member
    let member = create_test_user_and_login(&test_app.address).await;
    add_team_member_via_api(&test_app.address, &owner.token, &team_id, &member.username).await;

    // Member sends a message
    let message_data = json!({
        "message": "Member's message"
    });

    let response = client
        .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", member.token))
        .json(&message_data)
        .send()
        .await
        .expect("Failed to send chat message");

    let body: serde_json::Value = response.json().await.unwrap();
    let message_id = body["chat_message"]["id"].as_str().unwrap();

    // Owner (team owner has admin rights) deletes member's message
    let response = client
        .delete(&format!("{}/league/teams/{}/chat/{}", test_app.address, team_id, message_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("Failed to delete chat message");

    assert_eq!(response.status().as_u16(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap());
}

#[tokio::test]
async fn test_member_cannot_delete_others_message() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Add two members
    let member1 = create_test_user_and_login(&test_app.address).await;
    add_team_member_via_api(&test_app.address, &owner.token, &team_id, &member1.username).await;

    let member2 = create_test_user_and_login(&test_app.address).await;
    add_team_member_via_api(&test_app.address, &owner.token, &team_id, &member2.username).await;

    // Member1 sends a message
    let message_data = json!({
        "message": "Member1's message"
    });

    let response = client
        .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", member1.token))
        .json(&message_data)
        .send()
        .await
        .expect("Failed to send chat message");

    let body: serde_json::Value = response.json().await.unwrap();
    let message_id = body["chat_message"]["id"].as_str().unwrap();

    // Member2 tries to delete member1's message
    let response = client
        .delete(&format!("{}/league/teams/{}/chat/{}", test_app.address, team_id, message_id))
        .header("Authorization", format!("Bearer {}", member2.token))
        .send()
        .await
        .expect("Failed to delete chat message");

    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn test_message_validation_empty() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Try to send empty message
    let message_data = json!({
        "message": "   "
    });

    let response = client
        .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&message_data)
        .send()
        .await
        .expect("Failed to send chat message");

    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn test_message_validation_too_long() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Try to send message longer than 5000 characters
    let long_message = "a".repeat(5001);
    let message_data = json!({
        "message": long_message
    });

    let response = client
        .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&message_data)
        .send()
        .await
        .expect("Failed to send chat message");

    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn test_multiple_team_members_can_chat() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Add 3 members
    let mut members = vec![];
    for _ in 1..=3 {
        let member = create_test_user_and_login(&test_app.address).await;
        add_team_member_via_api(&test_app.address, &owner.token, &team_id, &member.username).await;
        members.push(member);
    }

    // Each member sends a message
    for (i, member) in members.iter().enumerate() {
        let message_data = json!({
            "message": format!("Message from member {}", i + 1)
        });

        let response = client
            .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
            .header("Authorization", format!("Bearer {}", member.token))
            .json(&message_data)
            .send()
            .await
            .expect("Failed to send chat message");
        assert_eq!(response.status().as_u16(), 200);
    }

    // Owner retrieves all messages
    let response = client
        .get(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("Failed to get chat history");

    assert_eq!(response.status().as_u16(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3);
}

#[tokio::test]
async fn test_websocket_receives_chat_message() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Add a member
    let member = create_test_user_and_login(&test_app.address).await;
    add_team_member_via_api(&test_app.address, &owner.token, &team_id, &member.username).await;

    // Connect member to WebSocket
    let ws_url = format!("{}/game-ws?token={}", test_app.address.replace("http", "ws"), member.token);
    let request = ws_url.into_client_request().expect("Failed to create request");
    let (mut ws_stream, _) = connect_async(request)
        .await
        .expect("Failed to connect to WebSocket");

    // Wait for welcome message
    let welcome_msg = ws_stream.next().await.expect("No welcome message").unwrap();
    match welcome_msg {
        Message::Text(text) => {
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(json["event_type"], "redis_subscriptions_ready");
        },
        _ => panic!("Expected text message"),
    };

    // Give WebSocket time to fully subscribe
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Owner sends a chat message via REST API
    let message_data = json!({
        "message": "Hello from owner!"
    });

    client
        .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&message_data)
        .send()
        .await
        .expect("Failed to send chat message");

    // Member should receive the message via WebSocket
    // May receive other events first (like player_pool events), so keep reading until we get our chat message
    let mut found_chat_message = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);

    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(1), ws_stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let json: serde_json::Value = serde_json::from_str(&text)
                    .expect("Failed to parse WebSocket message");

                if json["event_type"] == "team_chat_message" {
                    assert_eq!(json["message"], "Hello from owner!");
                    assert_eq!(json["username"], owner.username);
                    assert_eq!(json["team_id"], team_id);
                    found_chat_message = true;
                    break;
                }
                // Skip other events
            },
            Ok(Some(Ok(_))) => continue, // Skip non-text messages
            Ok(Some(Err(e))) => panic!("WebSocket error: {}", e),
            Ok(None) => panic!("WebSocket stream ended"),
            Err(_) => continue, // Timeout on this iteration, try again
        }
    }

    assert!(found_chat_message, "Never received team_chat_message event");

    ws_stream.send(Message::Close(None)).await.unwrap();
}

#[tokio::test]
async fn test_websocket_receives_edited_message() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Add a member
    let member = create_test_user_and_login(&test_app.address).await;
    add_team_member_via_api(&test_app.address, &owner.token, &team_id, &member.username).await;

    // Owner sends a message first
    let message_data = json!({
        "message": "Original message"
    });

    let response = client
        .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&message_data)
        .send()
        .await
        .expect("Failed to send chat message");

    let body: serde_json::Value = response.json().await.unwrap();
    let message_id = body["chat_message"]["id"].as_str().unwrap();

    // Connect member to WebSocket
    let ws_url = format!("{}/game-ws?token={}", test_app.address.replace("http", "ws"), member.token);
    let request = ws_url.into_client_request().expect("Failed to create request");
    let (mut ws_stream, _) = connect_async(request)
        .await
        .expect("Failed to connect to WebSocket");

    // Wait for welcome message
    let welcome_msg = ws_stream.next().await.expect("No welcome message").unwrap();
    match welcome_msg {
        Message::Text(text) => {
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(json["event_type"], "redis_subscriptions_ready");
        },
        _ => panic!("Expected text message"),
    };

    // Give WebSocket time to fully subscribe
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Owner edits the message
    let edit_data = json!({
        "message": "Edited message"
    });

    client
        .put(&format!("{}/league/teams/{}/chat/{}", test_app.address, team_id, message_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&edit_data)
        .send()
        .await
        .expect("Failed to edit message");

    // Member should receive the edit event via WebSocket
    // May receive other events first, so keep reading until we get our edit event
    let mut found_edit_event = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);

    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(1), ws_stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let json: serde_json::Value = serde_json::from_str(&text)
                    .expect("Failed to parse WebSocket message");

                if json["event_type"] == "team_chat_message_edited" {
                    assert_eq!(json["message"], "Edited message");
                    assert_eq!(json["message_id"], message_id);
                    assert_eq!(json["username"], owner.username);
                    found_edit_event = true;
                    break;
                }
            },
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(e))) => panic!("WebSocket error: {}", e),
            Ok(None) => panic!("WebSocket stream ended"),
            Err(_) => continue,
        }
    }

    assert!(found_edit_event, "Never received team_chat_message_edited event");

    ws_stream.send(Message::Close(None)).await.unwrap();
}

#[tokio::test]
async fn test_websocket_receives_deleted_message() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create owner and team
    let owner = create_test_user_and_login(&test_app.address).await;
    let team_id = create_team_via_league_api(&test_app.address, &owner.token).await;

    // Add a member
    let member = create_test_user_and_login(&test_app.address).await;
    add_team_member_via_api(&test_app.address, &owner.token, &team_id, &member.username).await;

    // Owner sends a message first
    let message_data = json!({
        "message": "Message to delete"
    });

    let response = client
        .post(&format!("{}/league/teams/{}/chat", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&message_data)
        .send()
        .await
        .expect("Failed to send chat message");

    let body: serde_json::Value = response.json().await.unwrap();
    let message_id = body["chat_message"]["id"].as_str().unwrap();

    // Connect member to WebSocket
    let ws_url = format!("{}/game-ws?token={}", test_app.address.replace("http", "ws"), member.token);
    let request = ws_url.into_client_request().expect("Failed to create request");
    let (mut ws_stream, _) = connect_async(request)
        .await
        .expect("Failed to connect to WebSocket");

    // Wait for welcome message
    let welcome_msg = ws_stream.next().await.expect("No welcome message").unwrap();
    match welcome_msg {
        Message::Text(text) => {
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(json["event_type"], "redis_subscriptions_ready");
        },
        _ => panic!("Expected text message"),
    };

    // Give WebSocket time to fully subscribe
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Owner deletes the message
    client
        .delete(&format!("{}/league/teams/{}/chat/{}", test_app.address, team_id, message_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("Failed to delete message");

    // Member should receive the delete event via WebSocket
    // May receive other events first, so keep reading until we get our delete event
    let mut found_delete_event = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);

    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(1), ws_stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let json: serde_json::Value = serde_json::from_str(&text)
                    .expect("Failed to parse WebSocket message");

                if json["event_type"] == "team_chat_message_deleted" {
                    assert_eq!(json["message_id"], message_id);
                    assert_eq!(json["team_id"], team_id);
                    found_delete_event = true;
                    break;
                }
            },
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(e))) => panic!("WebSocket error: {}", e),
            Ok(None) => panic!("WebSocket stream ended"),
            Err(_) => continue,
        }
    }

    assert!(found_delete_event, "Never received team_chat_message_deleted event");

    ws_stream.send(Message::Close(None)).await.unwrap();
}
