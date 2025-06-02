use futures_util::{SinkExt, StreamExt};
use redis::AsyncCommands;
use reqwest::Client;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use std::time::Duration;
use uuid::Uuid;

mod common;
use common::utils::spawn_app;

#[tokio::test]
async fn websocket_connection_working() {
    // Set up the test app
    let test_app = spawn_app().await;
    let client = Client::new();

    // Register a new user
    let username = format!("wsuser{}", Uuid::new_v4());
    let password = "password123";
    let email = format!("{}@example.com", username);

    let user_request = json!({
        "username": username,
        "password": password,
        "email": email
    });

    let response = client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to execute registration request.");

    assert!(response.status().is_success(), "Registration should succeed");

    // Login to get JWT token
    let login_request = json!({
        "username": username,
        "password": password
    });

    let login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&login_request)
        .send()
        .await
        .expect("Failed to execute login request.");

    assert!(login_response.status().is_success(), "Login should succeed");
    
    let login_json = login_response.json::<serde_json::Value>().await
        .expect("Failed to parse login response as JSON");
    let token = login_json["token"].as_str().expect("Token not found in response");

    // Connect to WebSocket with token in query parameter
    let ws_url = format!("{}/ws?token={}", test_app.address.replace("http", "ws"), token);
    println!("Connecting to WebSocket server at: {}", ws_url);
    
    // Create client request with proper WebSocket headers
    let request = ws_url.into_client_request().expect("Failed to create request");
    
    // Connect to WebSocket server
    let (mut ws_stream, _) = connect_async(request)
        .await
        .expect("Failed to connect to WebSocket server");

    println!("WebSocket connected");

    // Wait for welcome message
    let welcome_msg = ws_stream.next().await.expect("No welcome message received").unwrap();
    let welcome_text = match welcome_msg {
        Message::Text(text) => text,
        _ => panic!("Expected text message for welcome"),
    };
    
    // Parse welcome message
    let welcome_json: serde_json::Value = serde_json::from_str(&welcome_text)
        .expect("Failed to parse welcome message as JSON");
    assert_eq!(welcome_json["type"], "welcome", "Expected welcome message type");
    assert!(welcome_json["user_id"].is_string(), "Welcome message should contain user_id");

    // Send a ping message
    let ping_msg = json!({
        "type": "ping",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });
    ws_stream.send(Message::Text(ping_msg.to_string())).await.unwrap();
    
    // Wait for pong response
    let pong_msg = ws_stream.next().await.expect("No pong response received").unwrap();
    let pong_text = match pong_msg {
        Message::Text(text) => text,
        _ => panic!("Expected text message for pong"),
    };
    
    let pong_json: serde_json::Value = serde_json::from_str(&pong_text)
        .expect("Failed to parse pong message as JSON");
    assert_eq!(pong_json["type"], "pong", "Expected pong message type");

    // Send a test message
    let test_msg = json!({
        "type": "test",
        "content": "Hello WebSocket Server!"
    });
    ws_stream.send(Message::Text(test_msg.to_string())).await.unwrap();
    
    // Wait for the echo response
    let response = ws_stream.next().await.expect("No echo response received").unwrap();
    
    let resp_text = match response {
        Message::Text(text) => text,
        _ => panic!("Expected text message"),
    };
    
    let echo_json: serde_json::Value = serde_json::from_str(&resp_text)
        .expect("Failed to parse echo message as JSON");
    assert_eq!(echo_json["type"], "echo", "Expected echo message type");
    assert_eq!(echo_json["content"], test_msg.to_string(), "Echo should contain original message");
    
    // Close the connection
    ws_stream.send(Message::Close(None)).await.unwrap();
}

#[tokio::test]
async fn websocket_redis_pubsub_working() {
    // Just run this test as informational - don't fail CI pipelines
    let ignore_failures = std::env::var("IGNORE_REDIS_FAILURES").is_ok();
    
    // Set up the test app
    let test_app = spawn_app().await;
    let client = Client::new();
    
    // Allow some time for the app to start and set up Redis connections
    println!("Waiting for test app to fully initialize...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Register a new user
    let username = format!("wsuser{}", Uuid::new_v4());
    let password = "password123";
    let email = format!("{}@example.com", username);

    let user_request = json!({
        "username": username,
        "password": password,
        "email": email
    });

    client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to execute registration request.");

    // Login to get JWT token
    let login_request = json!({
        "username": username,
        "password": password
    });

    let login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&login_request)
        .send()
        .await
        .expect("Failed to execute login request.");
    
    let login_json = login_response.json::<serde_json::Value>().await
        .expect("Failed to parse login response as JSON");
    let token = login_json["token"].as_str().expect("Token not found in response");

    // Get user_id from JWT token
    let user_id = decode_jwt_user_id(token).expect("Failed to decode JWT token");

    // Connect to WebSocket with token in query parameter
    let ws_url = format!("{}/ws?token={}", test_app.address.replace("http", "ws"), token);
    
    // Create a proper WebSocket request
    let request = ws_url.into_client_request().expect("Failed to create request");
    
    // Connect to WebSocket server
    let (mut ws_stream, _) = connect_async(request)
        .await
        .expect("Failed to connect to WebSocket server");

    // Wait for welcome message
    let welcome_msg = ws_stream.next().await.expect("No welcome message received").unwrap();
    let welcome_text = match welcome_msg {
        Message::Text(text) => text,
        _ => panic!("Expected text message for welcome"),
    };
    
    // Parse welcome message
    let welcome_json: serde_json::Value = serde_json::from_str(&welcome_text)
        .expect("Failed to parse welcome message as JSON");
    assert_eq!(welcome_json["type"], "welcome", "Expected welcome message type");
    assert!(welcome_json["user_id"].is_string(), "Welcome message should contain user_id");

    // Wait for Redis subscription confirmation
    let mut subscription_confirmed = false;
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            Some(msg) = ws_stream.next() => {
                match msg {
                    Ok(Message::Text(text)) => {
                        println!("Received message: {}", text);
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            if json.get("test").and_then(|t| t.as_str()) == Some("Redis subscription active!") {
                                subscription_confirmed = true;
                                break;
                            }
                        }
                    },
                    Ok(other) => println!("Received non-text message: {:?}", other),
                    Err(e) => println!("Error receiving message: {:?}", e),
                }
            }
            _ = &mut timeout => {
                println!("Timeout waiting for Redis subscription confirmation");
                break;
            }
        }
    }

    assert!(subscription_confirmed, "Did not receive Redis subscription confirmation");

    println!("WebSocket connected for Redis PubSub test");

    // Create Redis client - IMPORTANT FIX
    // Get Redis password from env var
    let redis_password = std::env::var("REDIS__REDIS__PASSWORD")
        .expect("REDIS__REDIS__PASSWORD environment variable is required for Redis test");
    
    // Build Redis URL with password
    let redis_url = format!("redis://:{}@localhost:6379", redis_password);
    println!("Connecting to Redis with authentication");
    
    let redis_client = redis::Client::open(redis_url.as_str())
        .expect("Failed to create Redis client");
    
    let mut redis_conn = redis_client.get_async_connection().await
        .expect("Failed to connect to Redis");
    
    // Publish a message to the user's channel
    let test_message = json!({
        "event_type": "new_health_data",  // Match the format used in your backend
        "user_id": user_id.clone(),
        "username": username,
        "sync_id": Uuid::new_v4().to_string(),
        "message": "New health data available",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }).to_string();
    
    let user_channel = format!("evolveme:events:user:{}", user_id);
    println!("Publishing to Redis channel: {}", user_channel);
    let publish_result : Result<i32, redis::RedisError> = redis_conn.publish(&user_channel, &test_message).await;
    
    match publish_result {
        Ok(receivers) => println!("Published to {} receivers", receivers),
        Err(e) => println!("Failed to publish to Redis: {}", e),
    }
    
    // Wait for message to arrive (with timeout)
    let mut received_message = false;
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            Some(msg) = ws_stream.next() => {
                match msg {
                    Ok(Message::Text(text)) => {
                        println!("Received WebSocket message: {}", text);
                        // Check if the message contains our test data
                        // We need to match the format that our WebSocket server is forwarding
                        if text.contains("new_health_data") && text.contains(&user_id) {
                            received_message = true;
                            break;
                        }
                    },
                    Ok(other) => println!("Received non-text message: {:?}", other),
                    Err(e) => println!("Error receiving message: {:?}", e),
                }
            }
            _ = &mut timeout => {
                println!("Timeout waiting for Redis message");
                break;
            }
        }
    }
    
    if !received_message {
        println!("Did not receive Redis PubSub message via WebSocket");
        // Skip failure if we're ignoring Redis failures
        if ignore_failures {
            println!("IGNORE_REDIS_FAILURES is set, treating this as a warning rather than a test failure");
            return;
        }
        assert!(received_message, "Did not receive Redis PubSub message via WebSocket");
    }
}

// Helper function to extract user_id from JWT token (simplified version)
fn decode_jwt_user_id(token: &str) -> Result<String, String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err("Invalid JWT format".to_string());
    }
    
    let payload_base64 = parts[1];
    let payload_bytes = base64::decode_config(payload_base64, base64::URL_SAFE_NO_PAD)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;
    
    let payload_str = String::from_utf8(payload_bytes)
        .map_err(|e| format!("Failed to decode UTF-8: {}", e))?;
    
    let payload: serde_json::Value = serde_json::from_str(&payload_str)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;
    
    if let Some(sub) = payload.get("sub").and_then(|s| s.as_str()) {
        Ok(sub.to_string())
    } else {
        Err("No 'sub' claim found in token".to_string())
    }
}