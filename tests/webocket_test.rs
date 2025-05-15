use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use redis::AsyncCommands;
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

    // Connect to WebSocket with token authentication
    let ws_url = test_app.address.replace("http", "ws") + "/ws";
    
    // Create a proper WebSocket request with necessary headers
    let mut request = ws_url.clone().into_client_request()
        .expect("Failed to create request");
    
    // Add the authorization header
    request.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", token))
            .expect("Failed to create header value")
    );

    // Connect to WebSocket server
    println!("Connecting to WebSocket server at: {}", ws_url);
    let (mut ws_stream, response) = match connect_async(request).await {
        Ok(conn) => conn,
        Err(e) => {
            panic!("Failed to connect to WebSocket server: {:?}", e);
        }
    };

    println!("WebSocket connected with status: {}", response.status());

    // Send a message
    let msg = "Hello WebSocket Server!";
    ws_stream.send(Message::Text(msg.into())).await
        .expect("Failed to send message");
    
    // Wait for the response (should be an echo)
    let timeout = tokio::time::timeout(Duration::from_secs(5), ws_stream.next());
    let response = match timeout.await {
        Ok(Some(Ok(msg))) => msg,
        Ok(Some(Err(e))) => panic!("WebSocket error: {:?}", e),
        Ok(None) => panic!("WebSocket closed unexpectedly"),
        Err(_) => panic!("Timeout waiting for response"),
    };
    
    let resp_text = match response {
        Message::Text(text) => text,
        other => panic!("Expected text message, got: {:?}", other),
    };
    
    assert_eq!(resp_text, format!("Echo: {}", msg), "Server should echo back the message");
    
    // Close the connection cleanly
    ws_stream.send(Message::Close(None)).await
        .expect("Failed to send close message");
}

#[tokio::test]
async fn websocket_authentication_required() {
    // Set up the test app
    let test_app = spawn_app().await;
    
    // Try to connect without token
    let ws_url = test_app.address.replace("http", "ws") + "/ws";
    
    // Create a proper WebSocket request without authorization
    let request = ws_url.into_client_request()
        .expect("Failed to create request");

    // This should fail due to missing auth
    match connect_async(request).await {
        Ok(_) => panic!("WebSocket connection should fail without authentication"),
        Err(e) => {
            println!("Got expected error: {:?}", e);
            // We expect this to fail - test passes
        }
    }
}

#[tokio::test]
async fn websocket_invalid_token_rejected() {
    // Set up the test app
    let test_app = spawn_app().await;
    
    // Try to connect with invalid token
    let ws_url = test_app.address.replace("http", "ws") + "/ws";
    
    // Create request with invalid token
    let mut request = ws_url.into_client_request()
        .expect("Failed to create request");
    
    request.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str("Bearer invalid.token.here")
            .expect("Failed to create header value")
    );

    // This should fail due to invalid token
    match connect_async(request).await {
        Ok(_) => panic!("WebSocket connection should fail with invalid token"),
        Err(e) => {
            println!("Got expected error with invalid token: {:?}", e);
            // We expect this to fail - test passes
        }
    }
}

#[tokio::test]
async fn websocket_redis_pubsub_working() {
    // This test requires Redis to be running
    // Skip if Redis is not available in the test environment
    if std::env::var("TEST_WITH_REDIS").is_err() {
        println!("Skipping Redis PubSub test. Set TEST_WITH_REDIS environment variable to run this test.");
        return;
    }

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

    // Connect to WebSocket with token authentication
    let ws_url = test_app.address.replace("http", "ws") + "/ws";
    
    // Create a proper WebSocket request with necessary headers
    let mut request = ws_url.into_client_request()
        .expect("Failed to create request");
    
    // Add the authorization header
    request.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", token))
            .expect("Failed to create header value")
    );

    // Connect to WebSocket server
    let (mut ws_stream, _) = match connect_async(request).await {
        Ok(conn) => conn,
        Err(e) => {
            panic!("Failed to connect to WebSocket server: {:?}", e);
        }
    };

    println!("WebSocket connected for Redis PubSub test");

    // Create Redis client
    let redis_url = "redis://localhost:6379";
    let redis_client = redis::Client::open(redis_url).expect("Failed to create Redis client");
    let mut redis_conn = redis_client.get_async_connection().await.expect("Failed to connect to Redis");
    
    // Publish a message to the user's channel
    let test_message = json!({
        "event": "test_event",
        "data": "test_data",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }).to_string();
    
    let user_channel = format!("evolveme:events:user:{}", user_id);
    println!("Publishing to Redis channel: {}", user_channel);
    let publish_result: Result<i32, redis::RedisError> = redis_conn.publish(&user_channel, &test_message).await;
    
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
                        if text.contains("test_event") {
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
    
    // Close the connection
    let _ = ws_stream.send(Message::Close(None)).await;
    
    if std::env::var("IGNORE_REDIS_FAILURES").is_ok() {
        println!("Redis test failures are being ignored due to IGNORE_REDIS_FAILURES env var.");
        // Don't assert anything, just treating this as an informational test
    } else {
        assert!(received_message, "Did not receive Redis PubSub message via WebSocket");
    }
}

// Helper function to extract user_id from JWT token
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