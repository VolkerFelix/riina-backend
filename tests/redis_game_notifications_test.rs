// Test Redis pub/sub functionality for game evaluation notifications

use redis::{AsyncCommands, Client as RedisClient};
use reqwest::Client;
use secrecy::{SecretBox, ExposeSecret};
use serde_json::json;
use uuid::Uuid;
use chrono::{Weekday, NaiveTime, Utc};
use std::time::Duration;
use futures_util::StreamExt;
use sqlx;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request, get_next_date};
use common::admin_helpers::{create_admin_user_and_login, create_league_season};
use common::workout_data_helpers::{WorkoutData, WorkoutType, upload_workout_data_for_user};

use evolveme_backend::config::settings::get_config;

#[tokio::test]
async fn test_redis_game_evaluation_notifications() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🔍 Testing Redis pub/sub for game evaluation notifications");
    
    // Set up Redis connection for testing
    let config = get_config().expect("Failed to read config");
    let redis_url = format!("redis://:{}@localhost:{}", 
        config.redis.password.expose_secret(), 
        config.redis.port
    );
    let redis_client = RedisClient::open(redis_url).expect("Failed to create Redis client");
    let mut redis_conn = redis_client.get_async_connection().await.expect("Failed to connect to Redis");
    
    // Set up Redis subscription for global game events
    let mut pubsub_conn = redis_client.get_async_connection().await.expect("Failed to create pubsub connection");
    let mut pubsub = pubsub_conn.into_pubsub();
    
    // Subscribe to global game events channel
    pubsub.subscribe("game:events:global").await.expect("Failed to subscribe to global channel");
    println!("✅ Subscribed to Redis global game events channel");
    
    // Step 1: Set up test data (simplified version)
    let admin_user = create_admin_user_and_login(&app.address).await;
    let user1 = create_test_user_and_login(&app.address).await;
    let user2 = create_test_user_and_login(&app.address).await;
    
    // Upload health data
    upload_workout_data_for_user(&client, &app.address, &user1.token, &WorkoutData::new(WorkoutType::Intense, Utc::now(), 30)).await.unwrap();
    upload_workout_data_for_user(&client, &app.address, &user2.token, &WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30)).await.unwrap();
    
    let unique_suffix = &Uuid::new_v4().to_string()[..8];
    // Create league
    let league_request = json!({
        "name": format!("Redis Test League {}", unique_suffix),
        "description": "Testing Redis notifications",
        "max_teams": 2
    });
    
    let league_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", &app.address),
        &admin_user.token,
        Some(league_request),
    ).await;
    assert_eq!(league_response.status(), 201);
    
    let league_data = league_response.json::<serde_json::Value>().await.unwrap();
    let league_id = league_data["data"]["id"].as_str().unwrap();
    
    // Create teams
    let team_a_request = json!({
        "name": format!("Redis Team A {}", unique_suffix),
        "color": "#FF0000",
        "description": "Team A for Redis testing",
        "owner_id": user1.user_id
    });
    
    let team_a_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", &app.address),
        &admin_user.token,
        Some(team_a_request),
    ).await;
    assert_eq!(team_a_response.status(), 201);
    
    let team_a_data = team_a_response.json::<serde_json::Value>().await.unwrap();
    let team_a_id = team_a_data["data"]["id"].as_str().unwrap();
    
    let team_b_request = json!({
        "name": format!("Redis Team B {}", unique_suffix),
        "color": "#0000FF",
        "description": "Team B for Redis testing",
        "owner_id": user2.user_id
    });
    
    let team_b_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", &app.address),
        &admin_user.token,
        Some(team_b_request),
    ).await;
    assert_eq!(team_b_response.status(), 201);
    
    let team_b_data = team_b_response.json::<serde_json::Value>().await.unwrap();
    let team_b_id = team_b_data["data"]["id"].as_str().unwrap();
    
    // Note: user1 and user2 are already team members since they are the owners of their respective teams
    
    // Assign teams to league
    make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/teams", &app.address, league_id),
        &admin_user.token,
        Some(json!({"team_id": team_a_id})),
    ).await;
    
    make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/teams", &app.address, league_id),
        &admin_user.token,
        Some(json!({"team_id": team_b_id})),
    ).await;
    
    // Create season
    let start_date = get_next_date(Weekday::Sat, NaiveTime::from_hms_opt(22, 0, 0).unwrap());
    
    let season_name = format!("Redis Test Season {}", unique_suffix);
    let _season_id = create_league_season(
        &app.address,
        &admin_user.token,
        league_id,
        &season_name,
        &start_date.to_rfc3339()
    ).await;
    
    println!("✅ Test data setup complete");
    
    // Update games to current time before evaluation (like other tests)
    update_games_to_current_time(&app, league_id).await;
    
    // Trigger game cycle to start and finish games before evaluation
    println!("🔄 Running game management cycle...");
    let cycle_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/league/games/manage", &app.address),
        &admin_user.token,
        None,
    ).await;
    assert_eq!(cycle_response.status(), 200);
    let cycle_result = cycle_response.json::<serde_json::Value>().await.unwrap();
    println!("✅ Game cycle completed: {:?}", cycle_result);
    
    // Check game statuses after cycle
    let games_after_cycle = sqlx::query!(
        "SELECT id, status FROM games WHERE season_id IN (SELECT id FROM league_seasons WHERE league_id = $1)",
        Uuid::parse_str(league_id).expect("Invalid league ID")
    )
    .fetch_all(&app.db_pool)
    .await
    .expect("Failed to check game statuses after cycle");
    
    println!("📋 Game statuses after cycle:");
    for game in &games_after_cycle {
        println!("   Game {}: status='{}'", game.id, game.status);
    }
    
    // Wait for games to finish (their end time should be passed now)
    println!("⏳ Waiting for games to expire...");
    tokio::time::sleep(tokio::time::Duration::from_secs(12)).await;
    
    // Run game cycle again to finish expired games
    println!("🔄 Running game management cycle again to finish expired games...");
    let finish_cycle_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/league/games/manage", &app.address),
        &admin_user.token,
        None,
    ).await;
    assert_eq!(finish_cycle_response.status(), 200);
    let finish_cycle_result = finish_cycle_response.json::<serde_json::Value>().await.unwrap();
    println!("✅ Finish cycle completed: {:?}", finish_cycle_result);
    
    // Check final game statuses
    let games_final = sqlx::query!(
        "SELECT id, status FROM games WHERE season_id IN (SELECT id FROM league_seasons WHERE league_id = $1)",
        Uuid::parse_str(league_id).expect("Invalid league ID")
    )
    .fetch_all(&app.db_pool)
    .await
    .expect("Failed to check final game statuses");
    
    println!("📋 Final game statuses:");
    for game in &games_final {
        println!("   Game {}: status='{}'", game.id, game.status);
    }
    
    // Step 2: Subscribe to user-specific channels
    let user1_channel = format!("game:events:user:{}", user1.user_id);
    let user2_channel = format!("game:events:user:{}", user2.user_id);
    
    // Create separate Redis connections for user-specific subscriptions
    let mut user1_pubsub_conn = redis_client.get_async_connection().await.expect("Failed to create user1 pubsub");
    let mut user1_pubsub = user1_pubsub_conn.into_pubsub();
    user1_pubsub.subscribe(&user1_channel).await.expect("Failed to subscribe to user1 channel");
    
    let mut user2_pubsub_conn = redis_client.get_async_connection().await.expect("Failed to create user2 pubsub");
    let mut user2_pubsub = user2_pubsub_conn.into_pubsub();
    user2_pubsub.subscribe(&user2_channel).await.expect("Failed to subscribe to user2 channel");
    
    println!("✅ Subscribed to user-specific Redis channels");

    // Step 3: Trigger game evaluation
    println!("🎮 Triggering game evaluation...");
    
    // Use today's date since we updated games to current time
    let today = chrono::Utc::now().date_naive();
    let evaluation_request = json!({
        "date": today.to_string()
    });
    
    let evaluation_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/games/evaluate", &app.address),
        &admin_user.token,
        Some(evaluation_request),
    ).await;
    
    assert_eq!(evaluation_response.status(), 200);
    let eval_result = evaluation_response.json::<serde_json::Value>().await.unwrap();
    assert!(eval_result["success"].as_bool().unwrap_or(false));
    
    let games_evaluated = eval_result["games_evaluated"].as_u64().unwrap_or(0);
    println!("✅ Game evaluation completed: {} games evaluated", games_evaluated);
    
    if games_evaluated == 0 {
        println!("⚠️  No games were evaluated - checking for finished games in database");
    }
    
    // Step 4: Listen for Redis messages
    println!("🔍 Listening for Redis messages...");
    
    let mut global_messages_received = 0;
    let mut user1_messages_received = 0;
    let mut user2_messages_received = 0;
    
    // Set up message streams
    let mut global_stream = pubsub.on_message();
    let mut user1_stream = user1_pubsub.on_message();
    let mut user2_stream = user2_pubsub.on_message();
    
    // Listen for messages with timeout
    let timeout = Duration::from_secs(5);
    let start_time = std::time::Instant::now();
    
    while start_time.elapsed() < timeout {
        tokio::select! {
            // Global channel messages
            global_msg = global_stream.next() => {
                if let Some(msg) = global_msg {
                    if let Ok(payload) = msg.get_payload::<String>() {
                        println!("📨 Global message received: {}", payload);
                        
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload) {
                            let event_type = json["event_type"].as_str().unwrap_or("");
                            if event_type == "games_evaluated" {
                                global_messages_received += 1;
                                
                                // Verify message structure
                                assert!(json["total_games"].as_u64().unwrap_or(0) > 0);
                                assert!(json["game_results"].is_array());
                                assert!(json["standings_updated"].as_bool().unwrap_or(false));
                                
                                println!("✅ Valid games_evaluated event received on global channel");
                            }
                        }
                    }
                }
            },
            
            // User1 channel messages
            user1_msg = user1_stream.next() => {
                if let Some(msg) = user1_msg {
                    if let Ok(payload) = msg.get_payload::<String>() {
                        println!("📨 User1 message received: {}", payload);
                        
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload) {
                            let event_type = json["event_type"].as_str().unwrap_or("");
                            if event_type == "notification" {
                                user1_messages_received += 1;
                                
                                // Verify notification structure
                                assert_eq!(json["user_id"].as_str().unwrap_or(""), user1.user_id.to_string());
                                assert!(json["title"].as_str().unwrap_or("").contains("Match Result"));
                                assert_eq!(json["notification_type"].as_str().unwrap_or(""), "GameResult");
                                
                                println!("✅ Valid notification received for user1");
                            }
                        }
                    }
                }
            },
            
            // User2 channel messages
            user2_msg = user2_stream.next() => {
                if let Some(msg) = user2_msg {
                    if let Ok(payload) = msg.get_payload::<String>() {
                        println!("📨 User2 message received: {}", payload);
                        
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload) {
                            let event_type = json["event_type"].as_str().unwrap_or("");
                            if event_type == "notification" {
                                user2_messages_received += 1;
                                
                                // Verify notification structure
                                assert_eq!(json["user_id"].as_str().unwrap_or(""), user2.user_id.to_string());
                                assert!(json["title"].as_str().unwrap_or("").contains("Match Result"));
                                assert_eq!(json["notification_type"].as_str().unwrap_or(""), "GameResult");
                                
                                println!("✅ Valid notification received for user2");
                            }
                        }
                    }
                }
            },
            
            // Timeout
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                // Continue listening
            }
        }
        
        // Break if we've received all expected messages
        if global_messages_received >= 1 && user1_messages_received >= 1 && user2_messages_received >= 1 {
            break;
        }
    }
    
    // Step 5: Verify results
    println!("📊 Redis Message Summary:");
    println!("   🌐 Global messages: {}", global_messages_received);
    println!("   👤 User1 messages: {}", user1_messages_received);
    println!("   👤 User2 messages: {}", user2_messages_received);
    
    assert!(global_messages_received >= 1, "Should receive at least 1 global games_evaluated message");
    assert!(user1_messages_received >= 1, "Should receive at least 1 notification for user1");
    assert!(user2_messages_received >= 1, "Should receive at least 1 notification for user2");
    
    println!("✅ Redis pub/sub game evaluation notifications test completed successfully!");
    println!("🎉 All Redis messaging requirements verified!");
}

async fn update_games_to_current_time(app: &common::utils::TestApp, league_id: &str) {
    let now = chrono::Utc::now();
    let game_end = now + chrono::Duration::seconds(10); // Give more time for processing
    let league_uuid = uuid::Uuid::parse_str(league_id).expect("Invalid league ID");
    
    // Update all games in the league to current time for natural game cycle processing
    // Set game_start_time to now and game_end_time to 10 seconds later
    sqlx::query!(
        r#"
        UPDATE games 
        SET game_start_time = $1, game_end_time = $2
        WHERE season_id IN (
            SELECT id FROM league_seasons WHERE league_id = $3
        )
        "#,
        now,
        game_end,
        league_uuid.clone()
    )
    .execute(&app.db_pool)
    .await
    .expect("Failed to update games to current time");
    
    println!("✅ Updated games to current time - will be processed by game cycle");
    
    // Check game statuses right after update
    let games_check = sqlx::query!(
        "SELECT id, status, game_start_time, game_end_time FROM games WHERE season_id IN (SELECT id FROM league_seasons WHERE league_id = $1)",
        league_uuid
    )
    .fetch_all(&app.db_pool)
    .await
    .expect("Failed to check game statuses");
    
    println!("📋 Game statuses after update:");
    for game in &games_check {
        println!("   Game {}: status='{}', start={:?}, end={:?}", 
            game.id, game.status, game.game_start_time, game.game_end_time);
    }
    
    // Wait a moment for the times to be in the past
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
}