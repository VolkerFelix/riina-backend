//! Consolidated scheduler functionality tests
//! 
//! This test suite covers all scheduler operations including:
//! - SchedulerService lifecycle management (unit tests)
//! - Season scheduling and unscheduling (unit tests)
//! - Automated scheduler integration tests
//! - Game lifecycle management
//! - Multiple concurrent seasons
//! - Error recovery and cleanup
//! - Timezone handling

use http::status;
use serde_json::json;
use uuid::Uuid;
use reqwest::Client;
use chrono::{Weekday, NaiveTime, Utc, Duration};
use riina_backend::services::SchedulerService;
use riina_backend::config::redis::RedisSettings;
use riina_backend::config::settings::get_config;
use std::sync::Arc;
use secrecy::ExposeSecret;
use tokio::time::Duration as TokioDuration;

mod common;
use common::utils::{
    spawn_app,
    create_test_user_and_login,
    make_authenticated_request
};
use common::admin_helpers::{create_admin_user_and_login, create_teams_for_test, add_user_to_team};

// ============================================================================
// SCHEDULER SERVICE UNIT TESTS
// ============================================================================

#[tokio::test]
async fn test_scheduler_service_lifecycle() {
    println!("🧪 Testing SchedulerService Lifecycle");
    
    let app = spawn_app().await;
    let configuration = get_config().expect("Failed to read configuration.");
    let redis_client = Arc::new(redis::Client::open(RedisSettings::get_redis_url(&configuration.redis).expose_secret()).unwrap());
    
    // Create scheduler service
    let scheduler = SchedulerService::new_with_redis(
        app.db_pool.clone(), 
        redis_client.clone()
    ).await.expect("Failed to create scheduler service");
    
    // Test starting the scheduler
    scheduler.start().await.expect("Failed to start scheduler");
    println!("✅ Scheduler started successfully");
    
    // Test stopping the scheduler
    scheduler.stop().await.expect("Failed to stop scheduler");
    println!("✅ Scheduler stopped successfully");
    
    println!("🎉 Scheduler lifecycle test completed!");
}

#[tokio::test]
async fn test_scheduler_season_management() {
    println!("🧪 Testing SchedulerService Season Management");
    
    let app = spawn_app().await;
    let configuration = get_config().expect("Failed to read configuration.");
    let redis_client = Arc::new(redis::Client::open(RedisSettings::get_redis_url(&configuration.redis).expose_secret()).unwrap());
    
    let scheduler = SchedulerService::new_with_redis(
        app.db_pool.clone(), 
        redis_client.clone()
    ).await.expect("Failed to create scheduler service");
    
    scheduler.start().await.expect("Failed to start scheduler");
    
    let season1_id = Uuid::new_v4();
    let season2_id = Uuid::new_v4();
    let season3_id = Uuid::new_v4();
    
    // Schedule first season
    let result1 = scheduler.schedule_season(season1_id, "Test Season 1".to_string()).await;
    assert!(result1.is_ok(), "Should successfully schedule first season");
    println!("✅ Scheduled season 1");
    
    // Schedule second season
    let result2 = scheduler.schedule_season(season2_id, "Test Season 2".to_string()).await;
    assert!(result2.is_ok(), "Should successfully schedule second season");
    println!("✅ Scheduled season 2");
    
    // Schedule third season
    let result3 = scheduler.schedule_season(season3_id, "Test Season 3".to_string()).await;
    assert!(result3.is_ok(), "Should successfully schedule third season");
    println!("✅ Scheduled season 3");
    
    // Wait a moment to let jobs register
    tokio::time::sleep(TokioDuration::from_millis(100)).await;
    
    // Test unscheduling middle season
    let unschedule_result = scheduler.unschedule_season(season2_id).await;
    assert!(unschedule_result.is_ok(), "Should successfully unschedule season 2");
    println!("✅ Unscheduled season 2");
    
    // Test unscheduling non-existent season (should not error)
    let fake_season_id = Uuid::new_v4();
    let fake_unschedule_result = scheduler.unschedule_season(fake_season_id).await;
    assert!(fake_unschedule_result.is_ok(), "Should handle unscheduling non-existent season gracefully");
    println!("✅ Handled unscheduling non-existent season");
    
    // Test unscheduling remaining seasons
    let unschedule1_result = scheduler.unschedule_season(season1_id).await;
    assert!(unschedule1_result.is_ok(), "Should successfully unschedule season 1");
    
    let unschedule3_result = scheduler.unschedule_season(season3_id).await;
    assert!(unschedule3_result.is_ok(), "Should successfully unschedule season 3");
    println!("✅ Unscheduled remaining seasons");
    
    // Clean up
    scheduler.stop().await.expect("Failed to stop scheduler");
    println!("🎉 Season management test completed!");
}

#[tokio::test]
async fn test_scheduler_custom_frequency() {
    println!("🧪 Testing SchedulerService Custom Frequency");
    
    let app = spawn_app().await;
    let configuration = get_config().expect("Failed to read configuration.");
    let redis_client = Arc::new(redis::Client::open(RedisSettings::get_redis_url(&configuration.redis).expose_secret()).unwrap());
    
    let scheduler = SchedulerService::new_with_redis(
        app.db_pool.clone(), 
        redis_client.clone()
    ).await.expect("Failed to create scheduler service");
    
    scheduler.start().await.expect("Failed to start scheduler");
    
    let season_id = Uuid::new_v4();
    
    // Test scheduling with custom frequency (every 5 seconds for testing)
    let result = scheduler.schedule_season_with_frequency(
        season_id, 
        "Fast Test Season".to_string(), 
        "*/5 * * * * *"  // Every 5 seconds
    ).await;
    
    assert!(result.is_ok(), "Should successfully schedule season with custom frequency");
    println!("✅ Scheduled season with 5-second frequency");
    
    // Clean up
    scheduler.unschedule_season(season_id).await.expect("Should unschedule");
    scheduler.stop().await.expect("Failed to stop scheduler");
    
    println!("🎉 Custom frequency test completed!");
}

// ============================================================================
// AUTOMATED SCHEDULER INTEGRATION TESTS
// ============================================================================

#[tokio::test]
async fn test_automated_scheduler_game_lifecycle() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🤖 Testing Automated Scheduler - Game Lifecycle");
    
    // Create admin and users
    let admin_user = create_admin_user_and_login(&app.address).await;
    let user1 = create_test_user_and_login(&app.address).await;
    let user2 = create_test_user_and_login(&app.address).await;
    
    // Create league and teams
    let league_request = json!({
        "name": "Scheduler Test League",
        "description": "Testing automated scheduler",
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
    let league_data: serde_json::Value = league_response.json().await.unwrap();
    let league_id = league_data["data"]["id"].as_str().unwrap();
    
    // Create and assign 2 teams
    let team_ids = create_teams_for_test(&app.address, &admin_user.token, 2).await;
    for team_id in &team_ids {
        let assign_request = json!({"team_id": team_id});
        let assign_response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/leagues/{}/teams", &app.address, league_id),
            &admin_user.token,
            Some(assign_request),
        ).await;
        assert_eq!(assign_response.status(), 201);
    }
    
    // Add users to teams
    add_user_to_team(&app.address, &admin_user.token, &team_ids[0], user1.user_id).await;
    add_user_to_team(&app.address, &admin_user.token, &team_ids[1], user2.user_id).await;
    
    println!("✅ Created league with 2 teams and added users to teams");

    // Create season with 10-second games (for fast testing)
    let start_date = Utc::now() + Duration::seconds(10);
    
    let season_request = json!({
        "name": "Scheduler Test Season",
        "start_date": start_date.to_rfc3339(),
        "evaluation_timezone": "UTC",
        "auto_evaluation_enabled": true,
        "game_duration_minutes": 0.166, // 10 seconds = 0.166 minutes
        "evaluation_cron": "*/1 * * * * *" // Every 1 second to match short game duration
    });
    
    let season_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/seasons", &app.address, league_id),
        &admin_user.token,
        Some(season_request),
    ).await;
    
    assert_eq!(season_response.status(), 201);
    let season_data: serde_json::Value = season_response.json().await.unwrap();
    let season_id = season_data["data"]["id"].as_str().unwrap();
    
    println!("✅ Created season with auto-scheduling enabled (10-second games)");

    // Wait for scheduler to start the first game
    println!("⏳ Waiting for scheduler to start games...");
    let mut game_started = false;
    let mut attempts = 0;
    
    while !game_started && attempts < 45 { // Wait up to 45 seconds
        tokio::time::sleep(TokioDuration::from_secs(1)).await;
        attempts += 1;
        
        // Check for upcoming games first
        let upcoming_response = make_authenticated_request(
            &client,
            reqwest::Method::GET,
            &format!("{}/league/games/upcoming?season_id={}", &app.address, season_id),
            &admin_user.token,
            None,
        ).await;
        
        let mut upcoming_games = Vec::new();
        if upcoming_response.status() == 200 {
            let upcoming_data: serde_json::Value = upcoming_response.json().await.unwrap();
            upcoming_games = upcoming_data["data"].as_array().unwrap().to_vec();
        }
        
        // Also check for live games
        let live_response = make_authenticated_request(
            &client,
            reqwest::Method::GET,
            &format!("{}/league/games/live-active?season_id={}", &app.address, season_id),
            &admin_user.token,
            None,
        ).await;
        
        let mut live_games = Vec::new();
        if live_response.status() == 200 {
            let live_data: serde_json::Value = live_response.json().await.unwrap();
            live_games = live_data["data"].as_array().unwrap().to_vec();
        }
        
        if attempts % 10 == 0 || attempts < 5 { // Debug every 10 attempts or first 5
            println!("🔍 Debug (attempt {}): Found {} upcoming, {} live games", attempts, upcoming_games.len(), live_games.len());
            
            // Direct database query to see what's actually in the database
            let season_uuid = Uuid::parse_str(season_id).expect("Failed to parse season_id as UUID");
            let db_games = sqlx::query!(
                "SELECT id, status, game_start_time, game_end_time FROM games WHERE season_id = $1",
                season_uuid
            )
            .fetch_all(&app.db_pool)
            .await
            .expect("Failed to query games directly");

            println!("🔍 Direct DB query shows {} games:", db_games.len());
            for game in &db_games {
                println!("  - Game {}: status='{}', start={:?}, end={:?}", 
                    &game.id.to_string()[..8], game.status, game.game_start_time, game.game_end_time);
            }
            
            for (i, game) in upcoming_games.iter().enumerate() {
                let status = game["game"]["status"].as_str().unwrap_or("unknown");
                let start_time = game["game"]["game_start_time"].as_str().unwrap_or("none");
                println!("  Upcoming Game {}: status={}, start_time={}", i+1, status, start_time);
            }
            for (i, game) in live_games.iter().enumerate() {
                let status = game["game"]["status"].as_str().unwrap_or("unknown");
                let start_time = game["game"]["game_start_time"].as_str().unwrap_or("none");
                println!("  Live Game {}: status={}, start_time={}", i+1, status, start_time);
            }
            
            // If games disappeared, check all games for this season directly
            if upcoming_games.is_empty() && live_games.is_empty() && attempts > 5 {
                let all_games_response = make_authenticated_request(
                    &client,
                    reqwest::Method::GET,
                    &format!("{}/admin/games/status/{}", &app.address, season_id),
                    &admin_user.token,
                    None,
                ).await;
                if all_games_response.status() == 200 {
                    let all_games_data: serde_json::Value = all_games_response.json().await.unwrap();
                    println!("  🔍 Raw response: {}", serde_json::to_string_pretty(&all_games_data).unwrap());
                } else {
                    println!("  ❌ Failed to get games status, status: {}", all_games_response.status());
                }
            }
        }
        
        // Check if any live game is in progress
        for game in live_games {
            let status = game["game"]["status"].as_str().unwrap_or("");
            // Check for both possible status formats: "in_progress" (snake_case) and "InProgress" (PascalCase)
            if status == "in_progress" || status == "InProgress" {
                game_started = true;
                println!("✅ Scheduler started game automatically! Status: '{}' (attempt {})", status, attempts);
                break;
            }
        }
    }
    
    assert!(game_started, "Scheduler should have automatically started a game within 45 seconds");
    
    println!("🎉 Automated scheduler game lifecycle test completed successfully!");
}

#[tokio::test]
async fn test_scheduler_multiple_seasons() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🤖 Testing Automated Scheduler - Multiple Seasons");
    
    // Create admin
    let admin_user = create_admin_user_and_login(&app.address).await;
    
    // Create 2 leagues with different teams
    let league1_request = json!({
        "name": "League One",
        "description": "First test league",
        "max_teams": 2
    });
    
    let league2_request = json!({
        "name": "League Two", 
        "description": "Second test league",
        "max_teams": 2
    });
    
    let league1_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", &app.address),
        &admin_user.token,
        Some(league1_request),
    ).await;
    assert_eq!(league1_response.status(), 201);
    let league1_data: serde_json::Value = league1_response.json().await.unwrap();
    let league1_id = league1_data["data"]["id"].as_str().unwrap();
    
    let league2_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", &app.address),
        &admin_user.token,
        Some(league2_request),
    ).await;
    assert_eq!(league2_response.status(), 201);
    let league2_data: serde_json::Value = league2_response.json().await.unwrap();
    let league2_id = league2_data["data"]["id"].as_str().unwrap();
    
    // Create teams for both leagues
    let team_ids1 = create_teams_for_test(&app.address, &admin_user.token, 2).await;
    let team_ids2 = create_teams_for_test(&app.address, &admin_user.token, 2).await;
    
    // Assign teams to leagues
    for team_id in &team_ids1 {
        let assign_request = json!({"team_id": team_id});
        make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/leagues/{}/teams", &app.address, league1_id),
            &admin_user.token,
            Some(assign_request),
        ).await;
    }
    
    for team_id in &team_ids2 {
        let assign_request = json!({"team_id": team_id});
        make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/leagues/{}/teams", &app.address, league2_id),
            &admin_user.token,
            Some(assign_request),
        ).await;
    }
    
    println!("✅ Created 2 leagues with teams");
    
    // Create overlapping seasons with different game durations
    let start_date1 = Utc::now() + Duration::seconds(5);
    let start_date2 = Utc::now() + Duration::seconds(8); // Slight offset
    
    let season1_request = json!({
        "name": "Season One",
        "start_date": start_date1.to_rfc3339(),
        "evaluation_timezone": "UTC",
        "auto_evaluation_enabled": true,
        "game_duration_minutes": 0.083 // 5 seconds
    });
    
    let season2_request = json!({
        "name": "Season Two",
        "start_date": start_date2.to_rfc3339(),
        "evaluation_timezone": "UTC", 
        "auto_evaluation_enabled": true,
        "game_duration_minutes": 0.067 // 4 seconds
    });
    
    let season1_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/seasons", &app.address, league1_id),
        &admin_user.token,
        Some(season1_request),
    ).await;
    assert_eq!(season1_response.status(), 201);
    let season1_data: serde_json::Value = season1_response.json().await.unwrap();
    let season1_id = season1_data["data"]["id"].as_str().unwrap();
    
    let season2_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/seasons", &app.address, league2_id),
        &admin_user.token,
        Some(season2_request),
    ).await;
    assert_eq!(season2_response.status(), 201);
    let season2_data: serde_json::Value = season2_response.json().await.unwrap();
    let season2_id = season2_data["data"]["id"].as_str().unwrap();
    
    println!("✅ Created 2 concurrent seasons with different schedules");
    
    // Wait and verify both seasons are being managed
    println!("⏳ Waiting for both seasons to be processed by scheduler...");
    tokio::time::sleep(TokioDuration::from_secs(15)).await;
    
    // Check that both seasons have scheduled games
    let games1_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/schedule", &app.address, season1_id),
        &admin_user.token,
        None,
    ).await;
    assert_eq!(games1_response.status(), 200);
    
    let games2_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/schedule", &app.address, season2_id),
        &admin_user.token,
        None,
    ).await;
    assert_eq!(games2_response.status(), 200);
    
    let games1_data: serde_json::Value = games1_response.json().await.unwrap();
    let games2_data: serde_json::Value = games2_response.json().await.unwrap();
    
    let games1 = games1_data["data"]["games"].as_array().unwrap();
    let games2 = games2_data["data"]["games"].as_array().unwrap();
    
    assert!(!games1.is_empty(), "Season 1 should have scheduled games");
    assert!(!games2.is_empty(), "Season 2 should have scheduled games");
    
    println!("✅ Both seasons have scheduled games (Season1: {}, Season2: {})", 
             games1.len(), games2.len());
    
    println!("🎉 Multiple seasons scheduler test completed successfully!");
}

#[tokio::test]
async fn test_scheduler_error_recovery() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🤖 Testing Automated Scheduler - Error Recovery");
    
    // Create admin user
    let admin_user = create_admin_user_and_login(&app.address).await;
    
    // Create league
    let league_request = json!({
        "name": "Error Recovery League",
        "description": "Testing scheduler error recovery",
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
    let league_data: serde_json::Value = league_response.json().await.unwrap();
    let league_id = league_data["data"]["id"].as_str().unwrap();
    
    // Create and assign teams for the league
    let team_ids = create_teams_for_test(&app.address, &admin_user.token, 2).await;
    for team_id in &team_ids {
        let assign_request = json!({"team_id": team_id});
        let assign_response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/leagues/{}/teams", &app.address, league_id),
            &admin_user.token,
            Some(assign_request),
        ).await;
        assert_eq!(assign_response.status(), 201);
    }
    
    // Create season that will be deleted (to test unscheduling)
    let start_date = Utc::now() + Duration::minutes(1); // Future start
    
    let season_request = json!({
        "league_id": league_id,
        "name": "Season To Be Deleted",
        "start_date": start_date.to_rfc3339(),
        "team_ids": team_ids,
        "evaluation_timezone": "UTC",
        "auto_evaluation_enabled": true,
        "game_duration_minutes": 60 // 1 hour games
    });
    
    let season_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/seasons", &app.address, league_id),
        &admin_user.token,
        Some(season_request),
    ).await;
    assert_eq!(season_response.status(), 201);
    let season_data: serde_json::Value = season_response.json().await.unwrap();
    let season_id = season_data["data"]["id"].as_str().unwrap();
    
    println!("✅ Created season to be deleted");
    
    // Delete the season (should trigger unscheduling)
    let delete_response = make_authenticated_request(
        &client,
        reqwest::Method::DELETE,
        &format!("{}/admin/leagues/{}/seasons/{}", &app.address, league_id, season_id),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(delete_response.status(), 204);
    println!("✅ Deleted season - scheduler should have cleaned up the job");
    
    // Verify season is no longer listed
    let list_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/leagues/{}/seasons", &app.address, league_id),
        &admin_user.token,
        None,
    ).await;
    assert_eq!(list_response.status(), 200);
    
    let list_data: serde_json::Value = list_response.json().await.unwrap();
    
    // The API returns seasons directly under "data" as an array
    let seasons = list_data["data"].as_array().unwrap();
    
    // Should not find the deleted season
    let found_deleted_season = seasons.iter().any(|season| {
        season["id"].as_str().unwrap() == season_id
    });
    
    assert!(!found_deleted_season, "Deleted season should not appear in season list");
    
    // Create a new season to verify scheduler still works
    let new_start_date = Utc::now() + Duration::seconds(5);
    let new_season_request = json!({
        "league_id": league_id,
        "name": "Recovery Test Season",
        "start_date": new_start_date.to_rfc3339(),
        "team_ids": team_ids,
        "evaluation_timezone": "UTC",
        "auto_evaluation_enabled": true,
        "game_duration_minutes": 0.1 // 6 seconds
    });
    
    let new_season_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/seasons", &app.address, league_id),
        &admin_user.token,
        Some(new_season_request),
    ).await;
    
    let status = new_season_response.status();
    if status != 201 {
        let error_body = new_season_response.text().await.unwrap_or_default();
        panic!("Failed to create recovery season. Status: {}, Body: {}", status, error_body);
    }
    
    println!("✅ Created new season after deletion - scheduler should handle it normally");
    
    println!("🎉 Scheduler error recovery test completed successfully!");
}