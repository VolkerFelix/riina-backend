// Test to verify workout upload sends proper Redis/WebSocket notifications

use futures_util::StreamExt;
use redis::AsyncCommands;
use reqwest::Client;
use serde_json::json;
use chrono::{Utc, Duration};
use uuid::Uuid;
use std::time::Duration as StdDuration;
use secrecy::ExposeSecret;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};
use common::workout_data_helpers::{WorkoutData, WorkoutType, upload_workout_data_for_user};
use riina_backend::config::settings::get_config;

#[tokio::test]
async fn test_workout_upload_sends_redis_notifications() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing workout upload Redis notifications");
    
    // Set up Redis connection for testing
    let config = get_config().expect("Failed to read config");
    let redis_url = format!("redis://:{}@localhost:{}", 
        config.redis.password.expose_secret(), 
        config.redis.port
    );
    let redis_client = redis::Client::open(redis_url).expect("Failed to create Redis client");
    let mut pubsub_conn = redis_client.get_async_connection().await.expect("Failed to create pubsub connection");
    let mut pubsub = pubsub_conn.into_pubsub();
    
    // Create test user and health profile
    let test_user = create_test_user_and_login(&app.address).await;
    let user_uuid = test_user.user_id.to_string();
    
    // Create health profile for stats calculation
    let health_profile_data = json!({
        "age": 25,
        "gender": "male",
        "resting_heart_rate": 60
    });
    
    let profile_response = client
        .put(&format!("{}/profile/health_profile", &app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&health_profile_data)
        .send()
        .await
        .expect("Failed to create health profile");
    
    assert!(profile_response.status().is_success(), "Health profile creation should succeed");
    
    // Subscribe to user-specific channel
    let user_channel = format!("game:events:user:{}", user_uuid);
    pubsub.subscribe(&user_channel).await.expect("Failed to subscribe to user channel");
    
    // Also subscribe to global channel
    pubsub.subscribe("game:events:global").await.expect("Failed to subscribe to global channel");
    println!("✅ Subscribed to Redis channels");
    
    // Prepare workout data
    let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 30);
    
    // Upload workout data
    let upload_response = upload_workout_data_for_user(&client, &app.address, &test_user.token, &mut workout_data).await;
    
    assert_eq!(upload_response.is_ok(), true, "Workout upload should succeed");
    let upload_result = upload_response.unwrap();
    assert!(upload_result["success"].as_bool().unwrap_or(false));
    println!("✅ Workout uploaded successfully");
    
    // Listen for Redis messages
    println!("🔍 Listening for Redis notifications...");
    
    let mut user_notification_received = false;
    let mut global_notification_received = false;
    let mut messages_stream = pubsub.on_message();
    
    // Wait for messages with timeout
    let timeout = StdDuration::from_secs(5);
    let start_time = std::time::Instant::now();
    
    while start_time.elapsed() < timeout {
        // Try to receive a message
        if let Ok(Some(msg)) = tokio::time::timeout(StdDuration::from_millis(100), messages_stream.next()).await {
            if let Ok(payload) = msg.get_payload::<String>() {
                println!("📨 Received message on channel '{}': {}", msg.get_channel_name(), payload);
                
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload) {
                    let event_type = json["event_type"].as_str().unwrap_or("");
                    
                    if event_type == "workout_data_processed" {
                        // Verify message structure
                        assert_eq!(json["user_id"].as_str().unwrap_or(""), user_uuid.to_string());
                        assert_eq!(json["username"].as_str().unwrap_or(""), test_user.username);
                        assert!(json["sync_id"].is_string());
                        assert!(json["stat_changes"]["stamina_change"].is_number());
                        assert!(json["stat_changes"]["strength_change"].is_number());
                        assert!(json["timestamp"].is_string());
                        
                        if msg.get_channel_name() == user_channel {
                            user_notification_received = true;
                            println!("✅ User-specific workout notification received");
                        } else if msg.get_channel_name() == "game:events:global" {
                            global_notification_received = true;
                            println!("✅ Global workout notification received");
                        }
                    }
                }
            }
        }
        
        // Break if we've received both notifications
        if user_notification_received && global_notification_received {
            break;
        }
        
        // Small delay before next check
        tokio::time::sleep(StdDuration::from_millis(50)).await;
    }
    
    // Verify we received the notifications
    assert!(user_notification_received, "Should receive workout notification on user-specific channel");
    assert!(global_notification_received, "Should receive workout notification on global channel");
    
    println!("✅ All workout upload Redis notifications verified!");
    println!("🎉 Test completed successfully!");
}