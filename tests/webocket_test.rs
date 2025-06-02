// Fixed tests/websocket_test.rs - Properly handle Redis subscription timing

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
    let ws_url = format!("{}/game-ws?token={}", test_app.address.replace("http", "ws"), token);
    println!("Connecting to WebSocket server at: {}", ws_url);
    
    // Create client request with proper WebSocket headers
    let request = ws_url.into_client_request().expect("Failed to create request");
    
    // Connect to WebSocket server
    let (mut ws_stream, _) = connect_async(request)
        .await
        .expect("Failed to connect to WebSocket server");

    println!("WebSocket connected");

    // Wait for welcome message (player_joined)
    let welcome_msg = ws_stream.next().await.expect("No welcome message received").unwrap();
    let welcome_text = match welcome_msg {
        Message::Text(text) => {
            println!("Received welcome message: {}", text);
            text
        },
        _ => panic!("Expected text message for welcome"),
    };
    
    // Parse welcome message
    let welcome_json: serde_json::Value = serde_json::from_str(&welcome_text)
        .expect("Failed to parse welcome message as JSON");
    
    assert_eq!(welcome_json["event_type"], "player_joined", "Expected player_joined message type");
    assert!(welcome_json["user_id"].is_string(), "Welcome message should contain user_id");
    assert_eq!(welcome_json["username"], username, "Username should match");

    // Send a ping message
    let ping_msg = json!({
        "type": "ping",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });
    ws_stream.send(Message::Text(ping_msg.to_string())).await.unwrap();
    
    // Wait for pong response
    let pong_msg = ws_stream.next().await.expect("No pong response received").unwrap();
    let pong_text = match pong_msg {
        Message::Text(text) => {
            println!("Received pong message: {}", text);
            text
        },
        _ => panic!("Expected text message for pong"),
    };
    
    let pong_json: serde_json::Value = serde_json::from_str(&pong_text)
        .expect("Failed to parse pong message as JSON");
    assert_eq!(pong_json["type"], "pong", "Expected pong message type");

    // Test leaderboard request
    let leaderboard_request = json!({
        "type": "request_leaderboard",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });
    ws_stream.send(Message::Text(leaderboard_request.to_string())).await.unwrap();
    
    // Wait for leaderboard response
    let leaderboard_msg = ws_stream.next().await.expect("No leaderboard response received").unwrap();
    let leaderboard_text = match leaderboard_msg {
        Message::Text(text) => {
            println!("Received leaderboard message: {}", text);
            text
        },
        _ => panic!("Expected text message for leaderboard"),
    };
    
    let leaderboard_json: serde_json::Value = serde_json::from_str(&leaderboard_text)
        .expect("Failed to parse leaderboard message as JSON");
    assert_eq!(leaderboard_json["event_type"], "leaderboard_update", "Expected leaderboard_update message type");
    
    // Close the connection
    ws_stream.send(Message::Close(None)).await.unwrap();
}

#[tokio::test]
async fn websocket_redis_pubsub_working() {
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
    let ws_url = format!("{}/game-ws?token={}", test_app.address.replace("http", "ws"), token);
    
    // Create a proper WebSocket request
    let request = ws_url.into_client_request().expect("Failed to create request");
    
    // Connect to WebSocket server
    let (mut ws_stream, _) = connect_async(request)
        .await
        .expect("Failed to connect to WebSocket server");

    // Wait for welcome message (player_joined)
    let welcome_msg = ws_stream.next().await.expect("No welcome message received").unwrap();
    let welcome_text = match welcome_msg {
        Message::Text(text) => {
            println!("Received welcome message: {}", text);
            text
        },
        _ => panic!("Expected text message for welcome"),
    };
    
    // Parse welcome message
    let welcome_json: serde_json::Value = serde_json::from_str(&welcome_text)
        .expect("Failed to parse welcome message as JSON");
    assert_eq!(welcome_json["event_type"], "player_joined", "Expected player_joined message type");
    assert!(welcome_json["user_id"].is_string(), "Welcome message should contain user_id");

    println!("WebSocket connected for Redis PubSub test");

    // Create Redis client for testing
    let redis_password = std::env::var("REDIS__REDIS__PASSWORD")
        .expect("REDIS__REDIS__PASSWORD environment variable is required for Redis test");
    
    let redis_url = format!("redis://:{}@localhost:6379", redis_password);
    println!("Connecting to Redis with authentication");
    
    let redis_client = redis::Client::open(redis_url.as_str())
        .expect("Failed to create Redis client");
    
    let mut redis_conn = redis_client.get_async_connection().await
        .expect("Failed to connect to Redis");

    // Wait for Redis subscriptions to be established
    println!("Waiting for Redis subscriptions to be established...");
    
    let user_channel = format!("game:events:user:{}", user_id);
    let mut subscription_ready = false;
    let max_attempts = 15; // Reduce wait time since we know it works
    
    for attempt in 1..=max_attempts {
        println!("Checking subscription status (attempt {}/{})", attempt, max_attempts);
        
        // Check how many subscribers are on the user's channel
        let subscriber_check: Result<Vec<redis::Value>, redis::RedisError> = redis::cmd("PUBSUB")
            .arg("NUMSUB")
            .arg(&user_channel)
            .query_async(&mut redis_conn)
            .await;
        
        if let Ok(values) = subscriber_check {
            if values.len() >= 2 {
                if let redis::Value::Int(count) = &values[1] {
                    println!("Found {} subscribers for channel {}", count, user_channel);
                    if *count > 0 {
                        subscription_ready = true;
                        break;
                    }
                }
            }
        }
        
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    if !subscription_ready {
        panic!("Redis subscription was not established after {} seconds.", max_attempts);
    }

    println!("✅ Redis subscription confirmed! Publishing test message...");

    // Consume any pending messages (like redis_subscriptions_ready)
    println!("🧹 Clearing any pending system messages...");
    let clear_timeout = tokio::time::sleep(Duration::from_secs(2));
    tokio::pin!(clear_timeout);
    
    let mut system_messages = Vec::new();
    loop {
        tokio::select! {
            Some(msg) = ws_stream.next() => {
                if let Ok(Message::Text(text)) = msg {
                    println!("📋 System message: {}", text);
                    system_messages.push(text);
                }
            }
            _ = &mut clear_timeout => {
                break;
            }
        }
    }
    println!("Cleared {} system messages", system_messages.len());

    // NOW publish our test message
    let test_message = json!({
        "event_type": "health_data_processed",
        "user_id": user_id.clone(),
        "username": username.clone(),
        "sync_id": Uuid::new_v4().to_string(),
        "stat_changes": {
            "stamina_change": 5,
            "strength_change": 3,
            "wisdom_change": 2,
            "mana_change": 1,
            "experience_change": 100
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    }).to_string();
    
    println!("Publishing test message to Redis channel: {}", user_channel);
    println!("Test message content: {}", test_message);
    
    let publish_result : Result<i32, redis::RedisError> = redis_conn.publish(&user_channel, &test_message).await;
    
    match publish_result {
        Ok(receivers) => {
            println!("✅ Published to {} receivers", receivers);
            assert!(receivers > 0, "Should have at least 1 receiver");
        },
        Err(e) => panic!("Failed to publish to Redis: {}", e),
    }
    
    // Wait for OUR test message to arrive via WebSocket
    let mut received_our_message = false;
    let timeout = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(timeout);

    println!("🔍 Waiting for our test message to arrive via WebSocket...");
    
    loop {
        tokio::select! {
            Some(msg) = ws_stream.next() => {
                match msg {
                    Ok(Message::Text(text)) => {
                        println!("📥 Received WebSocket message: {}", text);
                        
                        // Parse the message to understand its structure
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                            println!("   📊 Parsed JSON: {:#}", parsed);
                            
                            // Check if this is our test message
                            if let Some(event_type) = parsed.get("event_type").and_then(|v| v.as_str()) {
                                if event_type == "health_data_processed" {
                                    if let Some(msg_user_id) = parsed.get("user_id").and_then(|v| v.as_str()) {
                                        if msg_user_id == user_id {
                                            received_our_message = true;
                                            println!("🎉 SUCCESS: Received our test message!");
                                            break;
                                        } else {
                                            println!("   ❌ User ID mismatch: got {}, expected {}", msg_user_id, user_id);
                                        }
                                    } else {
                                        println!("   ❌ No user_id in health_data_processed message");
                                    }
                                } else {
                                    println!("   ℹ️  Other event type: {}", event_type);
                                }
                            } else {
                                println!("   ❌ No event_type in message");
                            }
                        } else {
                            println!("   ❌ Failed to parse message as JSON");
                        }
                    },
                    Ok(other) => println!("📥 Received non-text message: {:?}", other),
                    Err(e) => println!("❌ Error receiving message: {:?}", e),
                }
            }
            _ = &mut timeout => {
                println!("❌ Timeout waiting for our test message");
                break;
            }
        }
    }
    
    if !received_our_message {
        println!("\n🔍 DEBUGGING INFO:");
        println!("Expected user_id: {}", user_id);
        println!("Expected message to contain: health_data_processed");
        println!("Published message: {}", test_message);
        println!("Channel used: {}", user_channel);
        
        // Try to understand what's happening - maybe messages are going to a different channel?
        println!("\n🔍 Let's try publishing to the global channel as a test...");
        let global_test_message = json!({
            "event_type": "test_global_message",
            "user_id": user_id.clone(),
            "message": "This is a global test message",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }).to_string();
        
        let global_result: Result<i32, redis::RedisError> = redis_conn.publish("game:events:global", &global_test_message).await;
        if let Ok(global_receivers) = global_result {
            println!("Published global test message to {} receivers", global_receivers);
            
            // Wait briefly for global message
            let global_timeout = tokio::time::sleep(Duration::from_secs(3));
            tokio::pin!(global_timeout);
            
            loop {
                tokio::select! {
                    Some(msg) = ws_stream.next() => {
                        if let Ok(Message::Text(text)) = msg {
                            println!("📥 Global test - received: {}", text);
                            if text.contains("test_global_message") {
                                println!("✅ Global channel IS working! Issue is with user-specific channel routing.");
                                break;
                            }
                        }
                    }
                    _ = &mut global_timeout => {
                        println!("❌ Global test also timed out");
                        break;
                    }
                }
            }
        }
    }
    
    assert!(received_our_message, "Did not receive our specific test message via WebSocket - Redis integration has an issue with message routing");
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