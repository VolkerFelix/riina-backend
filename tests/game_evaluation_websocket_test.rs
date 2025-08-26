// Comprehensive integration test for WebSocket notifications in game evaluations

use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use std::time::Duration;
use uuid::Uuid;
use chrono::{Weekday, NaiveTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use evolveme_backend::config::redis::RedisSettings;
use evolveme_backend::config::settings::get_config;
use secrecy::ExposeSecret;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request, get_next_date};
use common::admin_helpers::{create_admin_user_and_login, create_league_season};
use common::workout_data_helpers::{upload_workout_data_for_user, WorkoutData, WorkoutType};

#[tokio::test]
async fn test_game_evaluation_websocket_notifications_comprehensive() {
    let app = spawn_app().await;
    let client = Client::new();
    let configuration = get_config().expect("Failed to read configuration.");
    let redis_client = Arc::new(redis::Client::open(RedisSettings::get_redis_url(&configuration.redis).expose_secret()).unwrap());
    println!("🎯 Testing Comprehensive Game Evaluation WebSocket Notifications");
    
    // Step 1: Set up users with different power levels
    let admin_user = create_admin_user_and_login(&app.address).await;
    let user1 = create_test_user_and_login(&app.address).await; // Elite (Team A)
    let user2 = create_test_user_and_login(&app.address).await; // Advanced (Team A)  
    let user3 = create_test_user_and_login(&app.address).await; // Elite (Team B)
    let user4 = create_test_user_and_login(&app.address).await; // Advanced (Team B)
    
    println!("✅ Created 4 users + 1 admin");

    // Step 2: Upload health data to create power differences
    upload_workout_data_for_user(&client, &app.address, &user1.token, &WorkoutData::new(WorkoutType::Intense, Utc::now(), 30)).await.unwrap();
    upload_workout_data_for_user(&client, &app.address, &user2.token, &WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30)).await.unwrap();
    upload_workout_data_for_user(&client, &app.address, &user3.token, &WorkoutData::new(WorkoutType::Intense, Utc::now(), 30)).await.unwrap();
    upload_workout_data_for_user(&client, &app.address, &user4.token, &WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30)).await.unwrap();
    
    println!("✅ Uploaded health data for all users");

    // Step 3: Create league and teams
    let league_request = json!({
        "name": "WebSocket Test League",
        "description": "Testing WebSocket notifications for game evaluation",
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
    
    // Create Team A with unique name
    let unique_suffix = Uuid::new_v4().to_string().chars().take(8).collect::<String>();
    let team_a_request = json!({
        "name": format!("Elite Warriors {}", unique_suffix),
        "color": "#FF0000",
        "description": "Team with strong power",
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
    
    // Create Team B
    let team_b_request = json!({
        "name": format!("Advanced Fighters {}", unique_suffix),
        "color": "#0000FF", 
        "description": "Team with moderate power",
        "owner_id": user3.user_id
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
    
    // Add members to teams
    let add_user2_to_team_a = json!({
        "user_id": user2.user_id,
        "role": "member"
    });
    
    let member2_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams/{}/members", &app.address, team_a_id),
        &admin_user.token,
        Some(add_user2_to_team_a),
    ).await;
    assert_eq!(member2_response.status(), 201);
    
    let add_user4_to_team_b = json!({
        "user_id": user4.user_id,
        "role": "member"
    });
    
    let member4_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams/{}/members", &app.address, team_b_id),
        &admin_user.token,
        Some(add_user4_to_team_b),
    ).await;
    assert_eq!(member4_response.status(), 201);
    
    // Assign teams to league
    let assign_team_a = json!({"team_id": team_a_id});
    let assign_a_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/teams", &app.address, league_id),
        &admin_user.token,
        Some(assign_team_a),
    ).await;
    assert_eq!(assign_a_response.status(), 201);
    
    let assign_team_b = json!({"team_id": team_b_id});
    let assign_b_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/teams", &app.address, league_id),
        &admin_user.token,
        Some(assign_team_b),
    ).await;
    assert_eq!(assign_b_response.status(), 201);
    
    println!("✅ Created teams and assigned to league");

    // Step 4: Create a season with games for next Saturday at 10pm
    let start_date = get_next_date(Weekday::Sat, NaiveTime::from_hms_opt(22, 0, 0).unwrap());
    
    let _season_id = create_league_season(
        &app.address,
        &admin_user.token,
        league_id,
        "WebSocket Test Season",
        &start_date.to_rfc3339()
    ).await;
    
    println!("✅ Created season with games for next Saturday at 10pm");

    // Step 5: Set games to current time before evaluation
    update_games_to_current_time(&app, league_id).await;
    
    // Wait for games to complete their lifecycle (start → finish)
    let week_game_service = evolveme_backend::services::ManageGameService::new(app.db_pool.clone());
    
    println!("🔄 Running first game management cycle to start games...");
    let (_, _, started_games, _) = week_game_service.run_game_cycle().await.unwrap();
    println!("✅ First cycle completed: {} games started", started_games.len());
    
    if started_games.len() > 0 {
        println!("⏳ Waiting 6 seconds for games to finish...");
        tokio::time::sleep(tokio::time::Duration::from_secs(6)).await;
        
        println!("🔄 Running second game management cycle to finish games...");
        let (_, _, _, finished_games) = week_game_service.run_game_cycle().await.unwrap();
        println!("✅ Second cycle completed: {} games finished", finished_games.len());
    }

    // Step 6: Set up WebSocket connections for all users
    let mut websocket_connections = HashMap::new();
    
    // Connect user1 to WebSocket
    let user1_ws_url = format!("{}/game-ws?token={}", app.address.replace("http", "ws"), user1.token);
    let user1_request = user1_ws_url.into_client_request().expect("Failed to create user1 request");
    let (mut user1_ws, _) = connect_async(user1_request).await.expect("Failed to connect user1 WebSocket");
    
    // Wait for welcome message from user1
    let user1_welcome = user1_ws.next().await.expect("No welcome message for user1").unwrap();
    if let Message::Text(text) = user1_welcome {
        let welcome_json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(welcome_json["event_type"], "player_joined");
        println!("✅ User1 WebSocket connected and received welcome");
    }
    websocket_connections.insert("user1", user1_ws);
    
    // Connect user2 to WebSocket  
    let user2_ws_url = format!("{}/game-ws?token={}", app.address.replace("http", "ws"), user2.token);
    let user2_request = user2_ws_url.into_client_request().expect("Failed to create user2 request");
    let (mut user2_ws, _) = connect_async(user2_request).await.expect("Failed to connect user2 WebSocket");
    
    // Wait for welcome message from user2
    let user2_welcome = user2_ws.next().await.expect("No welcome message for user2").unwrap();
    if let Message::Text(text) = user2_welcome {
        let welcome_json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(welcome_json["event_type"], "player_joined");
        println!("✅ User2 WebSocket connected and received welcome");
    }
    websocket_connections.insert("user2", user2_ws);
    
    // Connect user3 to WebSocket
    let user3_ws_url = format!("{}/game-ws?token={}", app.address.replace("http", "ws"), user3.token);
    let user3_request = user3_ws_url.into_client_request().expect("Failed to create user3 request");
    let (mut user3_ws, _) = connect_async(user3_request).await.expect("Failed to connect user3 WebSocket");
    
    // Wait for welcome message from user3
    let user3_welcome = user3_ws.next().await.expect("No welcome message for user3").unwrap();
    if let Message::Text(text) = user3_welcome {
        let welcome_json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(welcome_json["event_type"], "player_joined");
        println!("✅ User3 WebSocket connected and received welcome");
    }
    websocket_connections.insert("user3", user3_ws);
    
    // Connect user4 to WebSocket
    let user4_ws_url = format!("{}/game-ws?token={}", app.address.replace("http", "ws"), user4.token);
    let user4_request = user4_ws_url.into_client_request().expect("Failed to create user4 request");
    let (mut user4_ws, _) = connect_async(user4_request).await.expect("Failed to connect user4 WebSocket");
    
    // Wait for welcome message from user4
    let user4_welcome = user4_ws.next().await.expect("No welcome message for user4").unwrap();
    if let Message::Text(text) = user4_welcome {
        let welcome_json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(welcome_json["event_type"], "player_joined");
        println!("✅ User4 WebSocket connected and received welcome");
    }
    websocket_connections.insert("user4", user4_ws);
    
    println!("✅ All WebSocket connections established");

    // Step 6: Wait for Redis subscriptions to be ready
    // Check for Redis subscription confirmation messages
    tokio::time::sleep(Duration::from_millis(1000)).await;
    
    // Drain any Redis subscription confirmation messages
    for (name, ws) in websocket_connections.iter_mut() {
        // Try to receive Redis subscription confirmation (non-blocking)
        if let Ok(Some(msg)) = tokio::time::timeout(Duration::from_millis(500), ws.next()).await {
            if let Ok(Message::Text(text)) = msg {
                let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                if json["event_type"] == "redis_subscriptions_ready" {
                    println!("✅ Redis subscriptions ready for {}", name);
                }
            }
        }
    }

    // Step 7: Trigger game evaluation via admin API and capture WebSocket notifications
    println!("🎮 Triggering game evaluation for date: {}", start_date.date_naive());
    
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
    assert!(eval_result["games_evaluated"].as_u64().unwrap_or(0) > 0);
    
    println!("✅ Game evaluation triggered successfully");

    // Step 8: Verify WebSocket notifications are received
    let mut global_events_received = 0;
    let mut individual_notifications_received = 0;
    
    println!("🔍 Checking for WebSocket notifications...");
    
    // Check each user's WebSocket connection for messages
    for (user_name, ws) in websocket_connections.iter_mut() {
        println!("Checking messages for {}", user_name);
        
        // Wait for potential messages with timeout
        let timeout_duration = Duration::from_secs(3);
        let start_time = std::time::Instant::now();
        
        while start_time.elapsed() < timeout_duration {
            // Try to receive a message with a short timeout
            if let Ok(Some(msg)) = tokio::time::timeout(Duration::from_millis(100), ws.next()).await {
                if let Ok(Message::Text(text)) = msg {
                    println!("📨 {} received message: {}", user_name, text);
                    
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        let event_type = json["event_type"].as_str().unwrap_or("");
                        
                        match event_type {
                            "games_evaluated" => {
                                global_events_received += 1;
                                println!("✅ Global games_evaluated event received by {}", user_name);
                                
                                // Verify event structure
                                assert!(json["total_games"].as_u64().unwrap_or(0) > 0);
                                assert!(json["game_results"].is_array());
                                assert!(json["standings_updated"].as_bool().unwrap_or(false));
                                assert!(json["evaluated_at"].is_string());
                            },
                            "notification" => {
                                individual_notifications_received += 1;
                                println!("✅ Individual notification received by {}", user_name);
                                
                                // Verify notification structure
                                assert!(json["title"].as_str().unwrap_or("").contains("Match Result"));
                                assert!(json["notification_type"].as_str().unwrap_or("") == "GameResult");
                                assert!(json["user_id"].is_string());
                                assert!(json["message"].is_string());
                                
                                let title = json["title"].as_str().unwrap_or("");
                                println!("   📋 Notification: {}", title);
                            },
                            "team_standings_updated" => {
                                println!("✅ Team standings update received by {}", user_name);
                                
                                // Verify standings structure
                                assert!(json["standings"].is_array());
                                assert!(json["league_name"].is_string());
                            },
                            _ => {
                                println!("   ℹ️  Other event: {}", event_type);
                            }
                        }
                    }
                }
            }
            
            // Small delay before next check
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    
    println!("📊 WebSocket Notification Summary:");
    println!("   🌐 Global events received: {}", global_events_received);
    println!("   👤 Individual notifications received: {}", individual_notifications_received);
    
    // Step 9: Verify expected notification counts
    // We expect:
    // - At least 1 global "games_evaluated" event (received by all connected users)
    // - At least 4 individual notifications (one for each team member)
    
    assert!(global_events_received >= 1, "Should receive at least 1 global games_evaluated event");
    assert!(individual_notifications_received >= 4, "Should receive at least 4 individual notifications (one per team member)");
    
    println!("✅ All WebSocket notification requirements verified!");
    
    println!("✅ Game evaluation and WebSocket notification integration test completed successfully!");
    println!("🎉 All assertions passed - WebSocket notifications are working correctly for game evaluations!");
}

async fn update_games_to_current_time(app: &common::utils::TestApp, league_id: &str) {
    let now = chrono::Utc::now();
    let game_end = now + chrono::Duration::seconds(5);
    let league_uuid = uuid::Uuid::parse_str(league_id).expect("Invalid league ID");
    
    // Update all games in the league to current time
    // Set game_start_time to now and game_end_time to 5 seconds later
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
        league_uuid
    )
    .execute(&app.db_pool)
    .await
    .expect("Failed to update game times to current time");
    
    println!("✅ Updated all games in league {} to current time with 5-second game duration", league_id);
}