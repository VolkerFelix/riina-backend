//! Automated scheduler integration tests
//! 
//! This test verifies that the automated scheduler correctly:
//! - Launches games at scheduled times
//! - Evaluates finished games automatically  
//! - Updates league standings
//! - Handles multiple concurrent seasons
//! - Recovers from errors gracefully

mod common;
use common::utils::{
    spawn_app,
    create_test_user_and_login,
    get_next_date,
    make_authenticated_request
};
use common::admin_helpers::{create_admin_user_and_login, create_teams_for_test, add_user_to_team};
use common::workout_data_helpers::{
    WorkoutData,
    WorkoutType,
    upload_workout_data_for_user
};

use serde_json::json;
use uuid::Uuid;
use reqwest::Client;
use chrono::{Weekday, NaiveTime, Utc, Duration};
use riina_backend::services::SchedulerService;
use std::sync::Arc;

#[tokio::test]
async fn test_automated_scheduler_game_lifecycle() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🤖 Testing Automated Scheduler - Game Lifecycle");
    
    // Step 1: Create admin and users
    let admin_user = create_admin_user_and_login(&app.address).await;
    let user1 = create_test_user_and_login(&app.address).await;
    let user2 = create_test_user_and_login(&app.address).await;
    
    // Step 2: Create league and teams
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

    // Step 3: Create season with 5-second games (for fast testing)
    let start_date = Utc::now() + Duration::seconds(10); // Start in 10 seconds
    
    let season_request = json!({
        "name": "Scheduler Test Season",
        "start_date": start_date.to_rfc3339(),
        "evaluation_timezone": "UTC",
        "auto_evaluation_enabled": true,
        "game_duration_minutes": 0.083 // 5 seconds = 0.083 minutes
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
    
    println!("✅ Created season with auto-scheduling enabled (5-second games)");

    // Step 4: Wait for scheduler to start the first game
    println!("⏳ Waiting for scheduler to start games...");
    let mut game_started = false;
    let mut attempts = 0;
    
    while !game_started && attempts < 30 { // Wait up to 30 seconds
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        attempts += 1;
        
        // Check for active games
        let games_response = make_authenticated_request(
            &client,
            reqwest::Method::GET,
            &format!("{}/league/games/upcoming?season_id={}", &app.address, season_id),
            &admin_user.token,
            None,
        ).await;
        
        if games_response.status() == 200 {
            let games_data: serde_json::Value = games_response.json().await.unwrap();
            let games = games_data["data"].as_array().unwrap();
            
            for game in games {
                let status = game["game"]["status"].as_str().unwrap_or("");
                if status == "in_progress" {
                    game_started = true;
                    println!("✅ Scheduler started game automatically! (attempt {})", attempts);
                    break;
                }
            }
        }
    }
    
    assert!(game_started, "Scheduler should have automatically started a game within 30 seconds");
    
    // Step 5: Wait for scheduler to finish and evaluate the game
    println!("⏳ Waiting for scheduler to finish and evaluate games...");
    let mut game_evaluated = false;
    attempts = 0;
    
    while !game_evaluated && attempts < 20 { // Wait up to 20 more seconds
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        attempts += 1;
        
        // Check standings to see if game was evaluated
        let standings_response = make_authenticated_request(
            &client,
            reqwest::Method::GET,
            &format!("{}/league/seasons/{}/standings", &app.address, season_id),
            &admin_user.token,
            None,
        ).await;
        
        if standings_response.status() == 200 {
            let standings_data: serde_json::Value = standings_response.json().await.unwrap();
            let standings = standings_data["data"]["standings"].as_array().unwrap();
            
            // Check if any team has games played > 0 (indicates game was evaluated)
            for team in standings {
                let games_played = team["games_played"].as_i64().unwrap_or(0);
                if games_played > 0 {
                    game_evaluated = true;
                    println!("✅ Scheduler evaluated game automatically! Team has {} games played (attempt {})", 
                             games_played, attempts);
                    break;
                }
            }
        }
    }
    
    assert!(game_evaluated, "Scheduler should have automatically evaluated a finished game within 20 seconds");
    
    println!("🎉 Automated scheduler test completed successfully!");
}

#[tokio::test]
async fn test_scheduler_multiple_seasons() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🤖 Testing Automated Scheduler - Multiple Seasons");
    
    // Step 1: Create admin
    let admin_user = create_admin_user_and_login(&app.address).await;
    
    // Step 2: Create 2 leagues with different teams
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
    
    // Step 3: Create overlapping seasons with different game durations
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
    
    // Step 4: Wait and verify both seasons are being managed
    println!("⏳ Waiting for both seasons to be processed by scheduler...");
    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
    
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
    
    // Step 1: Create admin user
    let admin_user = create_admin_user_and_login(&app.address).await;
    
    // Step 2: Create league
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
    
    // Step 3: Create season that will be deleted (to test unscheduling)
    let start_date = Utc::now() + Duration::minutes(1); // Future start
    
    let season_request = json!({
        "name": "Season To Be Deleted",
        "start_date": start_date.to_rfc3339(),
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
    
    // Step 4: Delete the season (should trigger unscheduling)
    let delete_response = make_authenticated_request(
        &client,
        reqwest::Method::DELETE,
        &format!("{}/admin/leagues/{}/seasons/{}", &app.address, league_id, season_id),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(delete_response.status(), 204);
    println!("✅ Deleted season - scheduler should have cleaned up the job");
    
    // Step 5: Verify season is no longer listed
    let list_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/leagues/{}/seasons", &app.address, league_id),
        &admin_user.token,
        None,
    ).await;
    assert_eq!(list_response.status(), 200);
    
    let list_data: serde_json::Value = list_response.json().await.unwrap();
    let seasons = list_data["data"]["seasons"].as_array().unwrap();
    
    // Should not find the deleted season
    let found_deleted_season = seasons.iter().any(|season| {
        season["id"].as_str().unwrap() == season_id
    });
    
    assert!(!found_deleted_season, "Deleted season should not appear in season list");
    
    // Step 6: Create a new season to verify scheduler still works
    let new_start_date = Utc::now() + Duration::seconds(5);
    let new_season_request = json!({
        "name": "Recovery Test Season",
        "start_date": new_start_date.to_rfc3339(),
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
    assert_eq!(new_season_response.status(), 201);
    
    println!("✅ Created new season after deletion - scheduler should handle it normally");
    
    println!("🎉 Scheduler error recovery test completed successfully!");
}

#[tokio::test]
async fn test_scheduler_timezone_handling() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🤖 Testing Automated Scheduler - Timezone Handling");
    
    // Step 1: Create admin user
    let admin_user = create_admin_user_and_login(&app.address).await;
    
    // Step 2: Create league
    let league_request = json!({
        "name": "Timezone Test League",
        "description": "Testing scheduler timezone handling",
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
    
    // Step 3: Create seasons with different timezones
    let utc_season_request = json!({
        "name": "UTC Season",
        "start_date": (Utc::now() + Duration::seconds(5)).to_rfc3339(),
        "evaluation_timezone": "UTC",
        "auto_evaluation_enabled": true,
        "game_duration_minutes": 0.083 // 5 seconds
    });
    
    let london_season_request = json!({
        "name": "London Season", 
        "start_date": (Utc::now() + Duration::seconds(8)).to_rfc3339(),
        "evaluation_timezone": "Europe/London",
        "auto_evaluation_enabled": true,
        "game_duration_minutes": 0.083 // 5 seconds
    });
    
    let utc_season_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/seasons", &app.address, league_id),
        &admin_user.token,
        Some(utc_season_request),
    ).await;
    assert_eq!(utc_season_response.status(), 201);
    
    let london_season_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/seasons", &app.address, league_id),
        &admin_user.token,
        Some(london_season_request),
    ).await;
    assert_eq!(london_season_response.status(), 201);
    
    let utc_season_data: serde_json::Value = utc_season_response.json().await.unwrap();
    let london_season_data: serde_json::Value = london_season_response.json().await.unwrap();
    
    // Verify timezone settings were saved correctly
    assert_eq!(utc_season_data["data"]["evaluation_timezone"].as_str().unwrap(), "UTC");
    assert_eq!(london_season_data["data"]["evaluation_timezone"].as_str().unwrap(), "Europe/London");
    
    println!("✅ Created seasons with different timezones (UTC and Europe/London)");
    
    // Step 4: Wait and verify both seasons are processed
    println!("⏳ Waiting for scheduler to process both timezone-specific seasons...");
    tokio::time::sleep(tokio::time::Duration::from_secs(12)).await;
    
    // Both seasons should be able to get their schedules (indicating successful processing)
    let utc_schedule_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/schedule", &app.address, utc_season_data["data"]["id"].as_str().unwrap()),
        &admin_user.token,
        None,
    ).await;
    assert_eq!(utc_schedule_response.status(), 200);
    
    let london_schedule_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/schedule", &app.address, london_season_data["data"]["id"].as_str().unwrap()),
        &admin_user.token,
        None,
    ).await;
    assert_eq!(london_schedule_response.status(), 200);
    
    println!("✅ Both timezone-specific seasons have valid schedules");
    
    println!("🎉 Scheduler timezone handling test completed successfully!");
}