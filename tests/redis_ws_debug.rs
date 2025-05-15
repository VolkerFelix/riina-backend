use futures_util::{SinkExt, StreamExt};
use redis::AsyncCommands;
use reqwest::Client;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use std::time::Duration;
use uuid::Uuid;

mod common;
use common::utils::spawn_app;

#[tokio::test]
async fn debug_redis_websocket_integration() {
    // Check for Redis password
    let redis_password = match std::env::var("REDIS__REDIS__PASSWORD") {
        Ok(pwd) => pwd,
        Err(_) => {
            println!("REDIS_PASSWORD environment variable is required.");
            return;
        }
    };

    println!("\n========== REDIS WEBSOCKET INTEGRATION DEBUG ==========\n");
    
    // Step 1: Check Redis connection directly
    println!("STEP 1: Testing direct Redis connection");
    let redis_url = format!("redis://:{}@localhost:6379", redis_password);
    
    let redis_client = match redis::Client::open(redis_url) {
        Ok(client) => {
            println!("√ Redis client created successfully");
            client
        },
        Err(e) => {
            println!("✗ Failed to create Redis client: {}", e);
            return;
        }
    };
    
    // Get a connection
    let mut redis_conn = match redis_client.get_async_connection().await {
        Ok(conn) => {
            println!("√ Redis connection established");
            conn
        },
        Err(e) => {
            println!("✗ Failed to connect to Redis: {}", e);
            return;
        }
    };
    
    // Step 2: Start test app
    println!("\nSTEP 2: Starting test application");
    let test_app = spawn_app().await;
    println!("√ Test app started at: {}", test_app.address);
    
    // Give app time to initialize
    println!("Waiting for app to fully initialize...");
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Step 3: Create test user
    println!("\nSTEP 3: Creating test user");
    let client = Client::new();
    let username = format!("wsuser{}", Uuid::new_v4());
    let password = "password123";
    let email = format!("{}@example.com", username);
    
    let register_response = client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&json!({
            "username": username,
            "password": password,
            "email": email
        }))
        .send()
        .await;
    
    match register_response {
        Ok(resp) if resp.status().is_success() => println!("√ User registered successfully"),
        Ok(resp) => {
            println!("✗ Registration failed: {}", resp.status());
            return;
        },
        Err(e) => {
            println!("✗ Registration request failed: {}", e);
            return;
        }
    }
    
    // Step 4: Login
    println!("\nSTEP 4: Logging in to get JWT token");
    let login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&json!({
            "username": username,
            "password": password
        }))
        .send()
        .await;
    
    let login_data = match login_response {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(data) => {
                    println!("√ Login successful");
                    data
                },
                Err(e) => {
                    println!("✗ Failed to parse login response: {}", e);
                    return;
                }
            }
        },
        Ok(resp) => {
            println!("✗ Login failed: {}", resp.status());
            return;
        },
        Err(e) => {
            println!("✗ Login request failed: {}", e);
            return;
        }
    };
    
    let token = login_data["token"].as_str().expect("Token not found in response");
    println!("√ Got JWT token");
    
    // Extract user_id from token
    let user_id = decode_jwt_user_id(token).expect("Failed to decode JWT token");
    println!("√ Extracted user_id: {}", user_id);
    
    // Step 5: Connect to WebSocket
    println!("\nSTEP 5: Connecting to WebSocket");
    let ws_url = test_app.address.replace("http", "ws") + "/ws";
    println!("WebSocket URL: {}", ws_url);
    
    let mut request = ws_url.into_client_request().expect("Failed to create request");
    request.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).expect("Failed to create header")
    );
    
    let (mut ws_stream, _) = match connect_async(request).await {
        Ok(conn) => {
            println!("√ WebSocket connection established");
            conn
        },
        Err(e) => {
            println!("✗ Failed to connect to WebSocket: {}", e);
            return;
        }
    };
    
    // Step 6: Test WebSocket echo
    println!("\nSTEP 6: Testing WebSocket echo functionality");
    let echo_msg = "Test echo message";
    
    match ws_stream.send(Message::Text(echo_msg.to_string())).await {
        Ok(_) => println!("√ Sent test message to WebSocket"),
        Err(e) => {
            println!("✗ Failed to send message: {}", e);
            return;
        }
    }
    
    // Wait for echo response
    let timeout = tokio::time::timeout(Duration::from_secs(3), ws_stream.next()).await;
    
    match timeout {
        Ok(Some(Ok(Message::Text(text)))) => {
            println!("√ Received response: {}", text);
            if text.contains(echo_msg) {
                println!("√ Echo functionality works");
            } else {
                println!("✗ Response doesn't match sent message");
            }
        },
        _ => {
            println!("✗ Failed to receive echo response");
            return;
        }
    }
    
    // Step 7: Check Redis subscribers
    println!("\nSTEP 7: Checking Redis channel subscribers");
    let user_channel = format!("evolveme:events:user:{}", user_id);
    println!("User channel: {}", user_channel);
    
    // Use PUBSUB CHANNELS to list active channels
    let channels: Vec<String> = redis::cmd("PUBSUB")
        .arg("CHANNELS")
        .query_async(&mut redis_conn)
        .await
        .expect("Failed to list Redis channels");
    
    println!("Active Redis channels: {:?}", channels);
    
    // Use PUBSUB NUMSUB to check subscriber count
    let (_, user_subs): (String, i32) = redis::cmd("PUBSUB")
        .arg("NUMSUB")
        .arg(&user_channel)
        .query_async(&mut redis_conn)
        .await
        .expect("Failed to get subscriber count");
    
    println!("User channel subscribers: {}", user_subs);
    
    if user_subs == 0 {
        println!("✗ No subscribers found on user channel");
        
        // Additional debug: List all active pubsub channels
        println!("\nListing all active Redis pubsub channels:");
        let all_channels: Vec<String> = redis::cmd("PUBSUB")
            .arg("CHANNELS")
            .arg("*")  // pattern to match all channels
            .query_async(&mut redis_conn)
            .await
            .expect("Failed to list Redis channels");
        
        println!("All channels: {:?}", all_channels);
    } else {
        println!("√ Found {} subscribers on user channel", user_subs);
    }
    
    // Step 8: Publish test message
    println!("\nSTEP 8: Publishing test message to Redis");
    
    // Create message with the exact format used in the backend
    let test_message = json!({
        "event_type": "new_health_data",
        "user_id": user_id.clone(),
        "username": username,
        "sync_id": Uuid::new_v4().to_string(),
        "message": "Test diagnostic message",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }).to_string();
    
    println!("Publishing to channel: {}", user_channel);
    println!("Message: {}", test_message);
    
    let pub_result: Result<i32, redis::RedisError> = redis_conn.publish(&user_channel, &test_message).await;
    
    match pub_result {
        Ok(receivers) => {
            println!("√ Published message to {} receivers", receivers);
            if receivers == 0 {
                println!("✗ No subscribers received the message");
            }
        },
        Err(e) => {
            println!("✗ Failed to publish message: {}", e);
            return;
        }
    };
    
    // Step 9: Check if WebSocket receives the message
    println!("\nSTEP 9: Waiting for WebSocket to receive the message");
    println!("(Waiting up to 10 seconds...)");
    
    let mut message_received = false;
    let mut read_timeout = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(read_timeout);
    
    loop {
        tokio::select! {
            Some(msg_result) = ws_stream.next() => {
                match msg_result {
                    Ok(Message::Text(text)) => {
                        println!("√ Received message from WebSocket: {}", text);
                        if text.contains("new_health_data") && text.contains(&user_id) {
                            println!("√ This is our test message!");
                            message_received = true;
                            break;
                        } else {
                            println!("This doesn't appear to be our test message, continuing to wait...");
                        }
                    },
                    Ok(other) => {
                        println!("Received non-text message: {:?}", other);
                    },
                    Err(e) => {
                        println!("✗ Error receiving message: {}", e);
                        break;
                    }
                }
            },
            _ = &mut read_timeout => {
                println!("✗ Timeout waiting for WebSocket message");
                break;
            }
        }
    }
    
    // Final summary
    println!("\n========== SUMMARY ==========");
    println!("Redis connection: Success");
    println!("Test app start: Success");
    println!("User registration and login: Success");
    println!("WebSocket connection: Success");
    println!("WebSocket echo: Success");
    println!("Redis subscribers on user channel: {}", user_subs);
    println!("Redis publish: {}", if pub_result.is_ok() { "Success" } else { "Failed" });
    println!("WebSocket received Redis message: {}", if message_received { "Success" } else { "Failed" });
    
    if !message_received {
        println!("\nPROBLEM DIAGNOSIS:");
        if user_subs == 0 {
            println!("The WebSocket server is not subscribing to the Redis channel.");
            println!("Check your WsConnection::started() method to ensure:");
            println!("1. The Redis connection is being established successfully");
            println!("2. The connection is converting to PubSub mode successfully");
            println!("3. The 'evolveme:events:user:{}' channel is being subscribed to", user_id);
            println!("4. Your Redis library version is compatible with your server");
        } else {
            println!("The WebSocket server is subscribed to Redis, but messages aren't forwarded to the client.");
            println!("Check your WebSocket implementation to ensure:");
            println!("1. Messages from Redis are being properly forwarded to the WebSocket client");
            println!("2. The message format you publish matches what your handler expects");
            println!("3. There are no errors in your message handling callback");
        }
    }
    
    // Clean up
    let _ = ws_stream.send(Message::Close(None)).await;
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