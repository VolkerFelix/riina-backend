use std::sync::Arc;

use riina_backend::config::redis::RedisSettings;
use riina_backend::config::settings::get_config;
use reqwest::Client;
use secrecy::ExposeSecret;
use serde_json::json;
use chrono::{Utc, Duration, Weekday, NaiveTime, DateTime};
use uuid::Uuid;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, TestApp, get_next_date, make_authenticated_request, UserRegLoginResponse, delete_test_user};
use common::admin_helpers::{create_admin_user_and_login, create_league_season, create_teams_for_test, create_league, add_team_to_league, add_user_to_team};
use common::live_game_helpers::*;
use common::workout_data_helpers::{WorkoutData, WorkoutType, upload_workout_data_for_user, create_health_profile_for_user};

#[tokio::test]
async fn test_complete_live_game_workflow() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let configuration = get_config().expect("Failed to read configuration.");
    let redis_client = Arc::new(redis::Client::open(RedisSettings::get_redis_url(&configuration.redis).expose_secret()).unwrap());

    // Step 1: Setup test environment - create league, season, teams, and users
    let live_game_environment = setup_live_game_environment(&test_app).await;
    
    // Update game times to current (games are auto-generated with future dates)
    update_game_times_to_now(&test_app, live_game_environment.first_game_id).await;
    
    start_test_game(&test_app, live_game_environment.first_game_id).await;

    let live_game = initialize_live_game(&test_app, live_game_environment.first_game_id, redis_client).await;
    
    // Verify initial live game state
    assert_eq!(live_game.home_score, 0);
    assert_eq!(live_game.away_score, 0);
    assert!(live_game.game_start_time.is_some());
    assert!(live_game.game_end_time.is_some());

    // Get season ID for the API call
    let season_id = get_season_id_for_game(&test_app, live_game_environment.first_game_id).await;
    
    // Fetch live games via API
    let live_games = get_live_games_via_api(&test_app, &client, &live_game_environment.home_user.token, Some(season_id)).await;
    
    // Verify our game is in the live games list
    assert!(!live_games.is_empty(), "Should have at least one live game");
    
    let our_game = live_games.iter().find(|g| g["game"]["id"].as_str() == Some(&live_game_environment.first_game_id.to_string()));
    assert!(our_game.is_some(), "Our game should be in the live games list");
    
    let api_game = our_game.unwrap();
    assert_eq!(api_game["game"]["status"].as_str(), Some("InProgress"));
    assert!(api_game["home_team_name"].is_string(), "Should have home team name");
    assert!(api_game["away_team_name"].is_string(), "Should have away team name");

    // Home team user uploads workout data
    let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 30);
    let response = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.home_user.token, &mut workout_data).await;
    assert!(response.is_ok(), "Workout upload should succeed");
    let response_data = response.unwrap();
    let stamina = response_data["data"]["game_stats"]["stamina_change"].as_i64().unwrap_or(0);
    let strength = response_data["data"]["game_stats"]["strength_change"].as_i64().unwrap_or(0);
    println!("DEBUG: Home user workout generated stamina: {}, strength: {}", stamina, strength);
    
    // Verify live game was updated
    let updated_live_game = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    println!("DEBUG: Updated live game - home_score: {}", updated_live_game.home_score);
    assert!(updated_live_game.home_score > 0, "Home team score should increase after workout upload");
    assert_eq!(updated_live_game.away_score, 0, "Away team score should remain 0");
    
    // Verify last scorer information
    assert_eq!(updated_live_game.last_scorer_id, Some(live_game_environment.home_user.user_id));
    assert_eq!(updated_live_game.last_scorer_name, Some(live_game_environment.home_user.username.clone()));
    assert_eq!(updated_live_game.last_scorer_team, Some("home".to_string()));

    // Away team users upload workout data
    let mut workout1 = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);
    let mut workout2 = WorkoutData::new(WorkoutType::Light, Utc::now(), 30);
    let response1 = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.away_user_1.token, &mut workout1).await;
    assert!(response1.is_ok(), "Workout upload should succeed");
    let response2 = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.away_user_2.token, &mut workout2).await;
    assert!(response2.is_ok(), "Workout upload should succeed");
    
    // Verify live game reflects both team activities
    let final_live_game = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    
    assert!(final_live_game.home_score > 0, "Home team should have score");
    assert!(final_live_game.away_score > 0, "Away team should have score after uploads");
    
    // For now, just check that both teams have scores
    assert!(final_live_game.home_score > 0, "Home team should have score");
    assert!(final_live_game.away_score > 0, "Away team should have score after uploads");

    // Test player contributions tracking
    let (home_contributions, away_contributions) = get_player_contributions(&test_app, final_live_game.id).await;
    
    // Verify home team contribution - filter to only the user we're testing
    let home_user_contrib = home_contributions.iter()
        .find(|c| c.user_id == live_game_environment.home_user.user_id)
        .expect("Home user should have a contribution record");
    assert!(home_user_contrib.total_score_contribution > 0);
    assert_eq!(home_user_contrib.contribution_count, 1);
    assert!(home_user_contrib.is_recently_active());

    // Verify away team contributions - filter to only users with actual contributions
    let away_active_contributions: Vec<&PlayerContribution> = away_contributions.iter()
        .filter(|c| c.total_score_contribution > 0)
        .collect();
    assert_eq!(away_active_contributions.len(), 2, "Both away users should have non-zero contributions");
    assert!(away_active_contributions.iter().any(|c| c.user_id == live_game_environment.away_user_1.user_id));
    assert!(away_active_contributions.iter().any(|c| c.user_id == live_game_environment.away_user_2.user_id));

    // Step 7: Test score events logging
    let score_events = get_recent_score_events(&test_app, final_live_game.id).await;
    assert_eq!(score_events.len(), 3, "Should have 3 score events (all users generated stats)");
    
    // Verify events are properly logged
    assert!(score_events.iter().any(|e| e.user_id == live_game_environment.home_user.user_id && e.team_side == "home"));
    assert!(score_events.iter().any(|e| e.user_id == live_game_environment.away_user_1.user_id && e.team_side == "away"));
    assert!(score_events.iter().any(|e| e.user_id == live_game_environment.away_user_2.user_id && e.team_side == "away"));

    // Step 8: Test game progress and time calculations
    assert!(final_live_game.game_progress() >= 0.0 && final_live_game.game_progress() <= 100.0);
    assert!(final_live_game.time_remaining().is_some());

    // Step 9: Test multiple uploads from same user (use different time to avoid duplicate detection)
    // Use a workout 30 minutes into the game (still within the 2-hour window)
    let second_workout_start = Utc::now() + Duration::minutes(30);
    let mut second_workout = WorkoutData::new(WorkoutType::Intense, second_workout_start, 30);
    let response = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.home_user.token, &mut second_workout).await;
    assert!(response.is_ok(), "Second workout upload should succeed");
    
    let after_second_upload = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert!(after_second_upload.home_score > final_live_game.home_score, 
        "Score should increase after second workout");

    // Verify contribution count increased
    let (updated_home_contributions, _) = get_player_contributions(&test_app, after_second_upload.id).await;
    let home_contrib = updated_home_contributions.iter()
        .find(|c| c.user_id == live_game_environment.home_user.user_id)
        .expect("Home user should have contributions");
    assert_eq!(home_contrib.contribution_count, 2, "Home user should have 2 contributions after second upload");

    // Step 10: Test live scoring history API endpoint
    test_live_scoring_history_api(&test_app, &client, &live_game_environment.home_user.token, live_game_environment.first_game_id, &live_game_environment.home_user, &live_game_environment.away_user_1, &live_game_environment.away_user_2).await;

    println!("✅ Live game integration test completed successfully!");
    println!("Final scores: {} {} - {} {}", 
        after_second_upload.home_team_name, 
        after_second_upload.home_score,
        after_second_upload.away_score, 
        after_second_upload.away_team_name
    );
}

#[tokio::test]
async fn test_live_game_edge_cases() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Test 1: Upload health data when no game is active
    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    
    // This should not crash or cause errors
    let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 30);
    let response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;
    assert!(response.is_ok(), "Workout upload should succeed");

    // Test 2: Multiple initializations of same live game
    let live_game_environment = setup_live_game_environment(&test_app).await;
    let configuration = get_config().expect("Failed to read configuration.");
    let redis_client = Arc::new(redis::Client::open(RedisSettings::get_redis_url(&configuration.redis).expose_secret()).unwrap());
    update_game_times_to_now(&test_app, live_game_environment.first_game_id).await;
    start_test_game(&test_app, live_game_environment.first_game_id).await;

    let live_game_1 = initialize_live_game(&test_app, live_game_environment.first_game_id, redis_client.clone()).await;
    let live_game_2 = initialize_live_game(&test_app, live_game_environment.first_game_id, redis_client).await;
    
    // Should return same live game, not create duplicate
    assert_eq!(live_game_1.id, live_game_2.id);

    println!("✅ Live game edge cases test completed successfully!");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_live_games_api_filtering() {
    let test_app = spawn_app().await;
    let client = Client::new();
    
    // Create admin and setup environment
    let admin = create_admin_user_and_login(&test_app.address).await;
    let league_id = create_league(&test_app.address, &admin.token, 2).await;
    let season_name = format!("API Test Season {}", &Uuid::new_v4().to_string()[..8]);
    
    // Create teams and add to league
    let user1 = create_test_user_and_login(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;
    
    let team_ids = create_teams_for_test(&test_app.address, &admin.token, 2).await;
    let team1_id = &team_ids[0];
    let team2_id = &team_ids[1];

    add_team_to_league(&test_app.address, &admin.token, &league_id, &team1_id).await;
    add_team_to_league(&test_app.address, &admin.token, &league_id, &team2_id).await;

    // Add users to teams
    add_user_to_team(&test_app.address, &admin.token, &team1_id, user1.user_id).await;
    add_user_to_team(&test_app.address, &admin.token, &team2_id, user2.user_id).await;

    // Create season
    let start_date = get_next_date(Weekday::Mon, NaiveTime::from_hms_opt(9, 0, 0).unwrap());
    let season_id = create_league_season(
        &test_app.address, 
        &admin.token, 
        &league_id, 
        &season_name, 
        &start_date.to_rfc3339()
    ).await;
    let season_uuid = Uuid::parse_str(&season_id).expect("Invalid season ID");
    
    // Create games with different statuses
    let scheduled_game_id = create_manual_game(&test_app, season_uuid, 
        Uuid::parse_str(&team1_id).unwrap(), 
        Uuid::parse_str(&team2_id).unwrap(), 
        "scheduled").await;
    
    let in_progress_game_id = create_manual_game(&test_app, season_uuid,
        Uuid::parse_str(&team1_id).unwrap(),
        Uuid::parse_str(&team2_id).unwrap(),
        "in_progress").await;
    
    let finished_game_id = create_manual_game(&test_app, season_uuid,
        Uuid::parse_str(&team1_id).unwrap(),
        Uuid::parse_str(&team2_id).unwrap(),
        "finished").await;
    
    // Fetch live games via API
    let live_games = get_live_games_via_api(&test_app, &client, &user1.token, Some(season_uuid)).await;
    
    // Verify only in_progress games are returned
    assert_eq!(live_games.len(), 1, "Should have exactly 1 live game");
    
    let live_game = &live_games[0];
    let expected_id = in_progress_game_id.to_string();
    assert_eq!(live_game["game"]["id"].as_str(), Some(expected_id.as_str()));
    assert_eq!(live_game["game"]["status"].as_str(), Some("InProgress"));
    
    // Verify scheduled and finished games are NOT in the list
    let has_scheduled = live_games.iter().any(|g| g["game"]["id"].as_str() == Some(&scheduled_game_id.to_string()));
    let has_finished = live_games.iter().any(|g| g["game"]["id"].as_str() == Some(&finished_game_id.to_string()));
    
    assert!(!has_scheduled, "Scheduled games should not appear in live games API");
    assert!(!has_finished, "Finished games should not appear in live games API");
    }

async fn create_manual_game(test_app: &TestApp, season_id: Uuid, home_team_id: Uuid, away_team_id: Uuid, status: &str) -> Uuid {
    let game_id = Uuid::new_v4();
    let now = Utc::now();
    
    sqlx::query!(
        r#"
        INSERT INTO games (id, season_id, home_team_id, away_team_id, game_start_time, game_end_time, week_number, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
        game_id,
        season_id,
        home_team_id,
        away_team_id,
        now,
        now + Duration::hours(2),
        1,
        status
    )
    .execute(&test_app.db_pool)
    .await
    .expect("Failed to create manual game");
    
    game_id
}

#[tokio::test]
async fn test_live_game_finish_workflow() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let configuration = get_config().expect("Failed to read configuration.");
    let redis_client = Arc::new(redis::Client::open(RedisSettings::get_redis_url(&configuration.redis).expose_secret()).unwrap());

    // Setup and create a game that ends soon
    let live_game_environment = setup_live_game_environment(&test_app).await;
    
    // Update the auto-generated game to end in 1 minute for testing
    update_game_to_short_duration(&test_app, live_game_environment.first_game_id).await;
    start_test_game(&test_app, live_game_environment.first_game_id).await;
    
    let live_game = initialize_live_game(&test_app, live_game_environment.first_game_id, redis_client.clone()).await;
    
    // Upload some data while game is active
    let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 30);
    let response = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.home_user.token, &mut workout_data).await;
    assert!(response.is_ok(), "Workout upload should succeed");
    
    let active_game = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert!(active_game.game_start_time.is_some());
    assert!(active_game.game_end_time.is_some());
    assert!(active_game.home_score > 0);

    // Wait for game to end (in real test, we'd manipulate time or end the game programmatically)
    finish_live_game(&test_app, live_game.id, redis_client).await;
    
    let finished_game = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    
    // Verify the game no longer appears in the live games API
    let season_id = get_season_id_for_game(&test_app, live_game_environment.first_game_id).await;
    let live_games_after_finish = get_live_games_via_api(&test_app, &client, &live_game_environment.home_user.token, Some(season_id)).await;
    
    // The finished game should NOT appear in the live games list
    let finished_game_in_api = live_games_after_finish.iter().find(|g| g["game"]["id"].as_str() == Some(&live_game_environment.first_game_id.to_string()));
    assert!(finished_game_in_api.is_none(), "Finished game should not appear in live games API");
    
    println!("✅ Verified finished game is removed from live games API");
    
    // Also verify through the actual game status in the API
    if !live_games_after_finish.is_empty() {
        // If there are any games, verify they're all actually live
        for game in &live_games_after_finish {
            let status = game["game"]["status"].as_str().unwrap_or("");
            assert!(status == "in_progress" || status == "live", 
                "Only in_progress or live games should be returned, got: {}", status);
        }
    }
    
    // Try to upload data after game ended - should not affect scores
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    let final_score = finished_game.home_score;
    let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 30);
    let response = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.home_user.token, &mut workout_data).await;
    assert!(response.is_ok(), "Workout upload should succeed");
    
    // Wait a bit for score recalculation
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Verify score didn't change
    let post_finish_game = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert_eq!(post_finish_game.home_score, final_score, "Score should not change after game ends");

    println!("✅ Live game finish workflow test completed successfully!");
}

#[tokio::test]
async fn test_workout_timing_validation_for_live_games() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let configuration = get_config().expect("Failed to read configuration.");
    let redis_client = Arc::new(redis::Client::open(RedisSettings::get_redis_url(&configuration.redis).expose_secret()).unwrap());  
    println!("🧪 Testing workout timing validation for live games...");

    // Setup test environment
    let live_game_environment = setup_live_game_environment(&test_app).await;
    
    // Update game times for precise testing
    update_game_times_to_now(&test_app, live_game_environment.first_game_id).await;
    start_test_game(&test_app, live_game_environment.first_game_id).await;
    
    let live_game = initialize_live_game(&test_app, live_game_environment.first_game_id, redis_client).await;
    let game_start = live_game.game_start_time.unwrap();
    let game_end = live_game.game_end_time.unwrap();
    
    println!("📅 Game window: {} to {}", game_start, game_end);
    
    // Test 1: Workout BEFORE game start - should NOT count
    println!("\n🔬 Test 1: Uploading workout from before game start...");
    let mut before_game_workout = WorkoutData::new(
        WorkoutType::Intense, 
        game_start - Duration::hours(2),
        30
    );
    
    let response = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.home_user.token, &mut before_game_workout).await;
    assert!(response.is_ok(), "Workout upload should succeed");
    
    // Check that score didn't increase
    let game_state_1 = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert_eq!(game_state_1.home_score, 0, "Score should not increase for workout before game start");
    println!("✅ Workout before game start correctly ignored");
    
    // Test 2: Workout DURING game - should count
    println!("\n🔬 Test 2: Uploading workout during game window...");
    let mut during_game_workout = WorkoutData::new(
        WorkoutType::Intense, 
        Utc::now(), 
        30
    );
    
    let response = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.home_user.token, &mut during_game_workout).await;
    assert!(response.is_ok(), "Workout upload should succeed");
    
    // Check that score increased
    let game_state_2 = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert!(game_state_2.home_score > 0, "Score should increase for workout during game");
    let score_during_game = game_state_2.home_score;
    println!("✅ Workout during game correctly counted: +{} points", score_during_game);
    
    // Test 3: Workout AFTER game end - should NOT count
    println!("\n🔬 Test 3: Uploading workout from after game end...");
    let mut after_game_workout = WorkoutData::new(
        WorkoutType::Intense, 
        game_end + Duration::hours(1),
        30
    );
    
    let response = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.home_user.token, &mut after_game_workout).await;
    assert!(response.is_ok(), "Workout upload should succeed");
    
    // Check that score didn't increase further
    let game_state_3 = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert_eq!(game_state_3.home_score, score_during_game, "Score should not increase for workout after game end");
    println!("✅ Workout after game end correctly ignored");
    
    // Test 4: Workout exactly at game start - should count
    println!("\n🔬 Test 4: Uploading workout exactly at game start...");
    let mut at_start_workout = WorkoutData::new(
        WorkoutType::Intense, 
        game_start,
        30
    );
    
    let response = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.away_user_1.token, &mut at_start_workout).await;
    assert!(response.is_ok(), "Workout upload should succeed");
    
    // Check that away team score increased
    let game_state_4 = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert!(game_state_4.away_score > 0, "Score should increase for workout at game start");
    println!("✅ Workout at game start correctly counted: +{} points", game_state_4.away_score);
    
    // Test 5: Workout exactly at game end - should NOT count (starts at boundary, ends after)
    println!("\n🔬 Test 5: Uploading workout exactly at game end...");
    let mut at_end_workout = WorkoutData::new(
        WorkoutType::Intense, 
        game_end,
        30
    );
    
    let response = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.away_user_1.token, &mut at_end_workout).await;
    assert!(response.is_ok(), "Workout upload should succeed");
    
    // Check that away team score did NOT increase
    let game_state_5 = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert_eq!(game_state_5.away_score, game_state_4.away_score, "Score should NOT increase for workout starting at game end");
    println!("✅ Workout starting at game end correctly ignored");
    
    println!("\n🎉 All workout timing validation tests passed!");
    println!("Final scores - Home: {}, Away: {}", game_state_5.home_score, game_state_5.away_score);
}

async fn test_live_scoring_history_api(
    test_app: &TestApp, 
    client: &Client, 
    token: &str, 
    game_id: Uuid,
    home_user: &UserRegLoginResponse,
    away_user_1: &UserRegLoginResponse,
    away_user_2: &UserRegLoginResponse
) {
    println!("🧪 Testing live scoring history API endpoint...");
    
    // Call the live game API endpoint that should include scoring events
    let url = format!("{}/league/games/{}/live", test_app.address, game_id);
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to call live game API");

    assert!(response.status().is_success(), "Live game API should return success");
    
    let response_data: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_data["success"], true, "API response should indicate success");
    
    // Verify the response structure matches what the frontend expects
    let data = response_data["data"].as_object().expect("Data should be an object");
    
    // Check required fields
    assert!(data.contains_key("game_id"), "Should contain game_id");
    assert!(data.contains_key("home_team_name"), "Should contain home_team_name");
    assert!(data.contains_key("away_team_name"), "Should contain away_team_name");
    assert!(data.contains_key("home_score"), "Should contain home_score");
    assert!(data.contains_key("away_score"), "Should contain away_score");
    assert!(data.contains_key("status"), "Should contain status");
    
    // Most importantly, check scoring_events
    assert!(data.contains_key("scoring_events"), "Should contain scoring_events array");
    
    let scoring_events = data["scoring_events"].as_array().expect("scoring_events should be an array");
    assert!(!scoring_events.is_empty(), "Should have scoring events from the test uploads");
    
    // Verify scoring event structure matches frontend expectations
    for event in scoring_events {
        let event_obj = event.as_object().expect("Each scoring event should be an object");
        
        // Check all required fields for frontend parsing
        assert!(event_obj.contains_key("id"), "Event should have id");
        assert!(event_obj.contains_key("user_id"), "Event should have user_id");
        assert!(event_obj.contains_key("username"), "Event should have username");
        assert!(event_obj.contains_key("team_id"), "Event should have team_id");
        assert!(event_obj.contains_key("team_side"), "Event should have team_side");
        assert!(event_obj.contains_key("score_points"), "Event should have score_points");
        assert!(event_obj.contains_key("description"), "Event should have description");
        assert!(event_obj.contains_key("occurred_at"), "Event should have occurred_at timestamp");
        
        // Verify team_side is valid
        let team_side = event_obj["team_side"].as_str().expect("team_side should be a string");
        assert!(team_side == "home" || team_side == "away", "team_side should be 'home' or 'away'");
        
        // Verify score_points is positive (since our test uploads should generate points)
        let score_points = event_obj["score_points"].as_f64().expect("score_points should be a number");
        assert!(score_points > 0.0, "Test uploads should generate positive points");
        
        // Verify the user_id matches one of our test users
        let event_user_id = event_obj["user_id"].as_str().expect("user_id should be a string");
        let event_user_uuid = Uuid::parse_str(event_user_id).expect("user_id should be valid UUID");
        assert!(
            event_user_uuid == home_user.user_id || event_user_uuid == away_user_1.user_id || event_user_uuid == away_user_2.user_id,
            "Event should be from one of our test users"
        );

        // Verify workout_details are present and properly structured
        assert!(event_obj.contains_key("workout_details"), "Event should have workout_details");
        
        if let Some(workout_details) = event_obj["workout_details"].as_object() {
            // Check that essential workout detail fields are present
            assert!(workout_details.contains_key("id"), "workout_details should have id");
            assert!(workout_details.contains_key("workout_date"), "workout_details should have workout_date");
            assert!(workout_details.contains_key("workout_start"), "workout_details should have workout_start");
            assert!(workout_details.contains_key("workout_end"), "workout_details should have workout_end");
            assert!(workout_details.contains_key("stamina_gained"), "workout_details should have stamina_gained");
            assert!(workout_details.contains_key("strength_gained"), "workout_details should have strength_gained");
            
            // For our test data that includes heart rate, verify those fields are present
            assert!(workout_details.contains_key("duration_minutes"), "workout_details should have duration_minutes");
            assert!(workout_details.contains_key("avg_heart_rate"), "workout_details should have avg_heart_rate");
            assert!(workout_details.contains_key("max_heart_rate"), "workout_details should have max_heart_rate");
            assert!(workout_details.contains_key("heart_rate_zones"), "workout_details should have heart_rate_zones");
            
            // Verify that the workout_details have actual values (not all null)
            // Our test workouts should have duration since they have start/end times
            if let (Some(start), Some(end)) = (workout_details["workout_start"].as_str(), workout_details["workout_end"].as_str()) {
                let workout_start = chrono::DateTime::parse_from_rfc3339(start).expect("Should parse workout_start");
                let workout_end = chrono::DateTime::parse_from_rfc3339(end).expect("Should parse workout_end");
                let expected_duration_minutes = (workout_end - workout_start).num_minutes();
                
                if expected_duration_minutes > 0 {
                    // The database should have the calculated duration, not null
                    assert!(
                        !workout_details["duration_minutes"].is_null(),
                        "workout_details.duration_minutes should not be null when workout has start/end times"
                    );
                    
                    let actual_duration = workout_details["duration_minutes"].as_i64()
                        .expect("duration_minutes should be a number, not null");
                    assert!(
                        actual_duration > 0,
                        "workout_details should have calculated duration_minutes > 0, got: {}",
                        actual_duration
                    );
                    
                    // Verify the calculated duration is reasonable (within 1 minute of expected)
                    let duration_diff = (actual_duration - expected_duration_minutes).abs();
                    assert!(
                        duration_diff <= 1,
                        "Calculated duration {} should be close to expected {}", 
                        actual_duration, expected_duration_minutes
                    );
                }
            }
            
            // Also verify heart rate data is properly calculated if present
            if workout_details.contains_key("avg_heart_rate") && !workout_details["avg_heart_rate"].is_null() {
                let avg_hr = workout_details["avg_heart_rate"].as_f64()
                    .expect("avg_heart_rate should be a number if not null");
                assert!(avg_hr > 0.0, "avg_heart_rate should be positive, got: {}", avg_hr);
                assert!(avg_hr < 300.0, "avg_heart_rate should be reasonable, got: {}", avg_hr);
            }
            
            // Verify heart rate zones are properly calculated and stored
            // Our test workouts include heart rate data, so zones should be calculated
            assert!(
                !workout_details["heart_rate_zones"].is_null(),
                "heart_rate_zones should not be null when workout has heart rate data"
            );
            
            if let Some(zones) = workout_details["heart_rate_zones"].as_array() {
                assert!(!zones.is_empty(), "heart_rate_zones should contain zone data when heart rate is present");
                
                // Verify zone structure
                for zone in zones {
                    let zone_obj = zone.as_object().expect("Each zone should be an object");
                    assert!(zone_obj.contains_key("zone"), "Zone should have 'zone' field");
                    assert!(zone_obj.contains_key("minutes"), "Zone should have 'minutes' field");
                    assert!(zone_obj.contains_key("stamina_gained"), "Zone should have 'stamina_gained' field");
                    assert!(zone_obj.contains_key("strength_gained"), "Zone should have 'strength_gained' field");
                    
                    // Verify zone has reasonable values
                    let zone_name = zone_obj["zone"].as_str().expect("zone should be a string");
                    assert!(
                        ["Zone1", "Zone2", "Zone3", "Zone4", "Zone5"].contains(&zone_name) || ["Rest", "Easy", "Moderate", "Hard"].contains(&zone_name),
                        "Zone name should be valid, got: {}", zone_name
                    );
                    
                    let minutes = zone_obj["minutes"].as_f64().expect("minutes should be a number");
                    assert!(minutes >= 0.0, "Zone minutes should be non-negative, got: {}", minutes);
                }
                
                // Verify that total zone minutes roughly equals workout duration
                let total_zone_minutes: f64 = zones.iter()
                    .map(|z| z["minutes"].as_f64().unwrap_or(0.0))
                    .sum();
                
                if let Some(duration) = workout_details["duration_minutes"].as_i64() {
                    let duration_diff = (total_zone_minutes - duration as f64).abs();
                    assert!(
                        duration_diff < 2.0,
                        "Total zone minutes {} should roughly equal workout duration {}",
                        total_zone_minutes, duration
                    );
                }
            } else {
                panic!("heart_rate_zones should be an array when heart rate data is present");
            }
            
            println!("✅ Workout details verified for event {}", event_obj["id"].as_str().unwrap_or("unknown"));
        } else {
            panic!("workout_details should be an object, not null");
        }
    }
    
    // Check that events are ordered by most recent first (as expected by frontend)
    if scoring_events.len() > 1 {
        for i in 0..scoring_events.len()-1 {
            let current_time = scoring_events[i]["occurred_at"].as_str().expect("Should have timestamp");
            let next_time = scoring_events[i+1]["occurred_at"].as_str().expect("Should have timestamp");
            
            let current_dt = chrono::DateTime::parse_from_rfc3339(current_time).expect("Should parse timestamp");
            let next_dt = chrono::DateTime::parse_from_rfc3339(next_time).expect("Should parse timestamp");
            
            assert!(current_dt >= next_dt, "Events should be ordered by most recent first");
        }
    }
    
    println!("✅ Live scoring history API test passed!");
    println!("   - Found {} scoring events", scoring_events.len());
    println!("   - All required fields present and valid");
    println!("   - Events properly ordered by timestamp");
}

async fn get_first_game_for_teams(test_app: &TestApp, season_id: Uuid, home_team_id: Uuid, away_team_id: Uuid) -> Uuid {
    // Get the auto-generated game between these teams
    let game = sqlx::query!(
        r#"
        SELECT id 
        FROM games 
        WHERE season_id = $1 
        AND ((home_team_id = $2 AND away_team_id = $3) OR (home_team_id = $3 AND away_team_id = $2))
        ORDER BY week_number
        LIMIT 1
        "#,
        season_id,
        home_team_id,
        away_team_id
    )
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to find auto-generated game");
    
    game.id
}

async fn create_test_game(test_app: &TestApp, client: &Client, home_team_id: Uuid, away_team_id: Uuid, season_id: Uuid) -> Uuid {
    let game_start = Utc::now();
    let game_end = game_start + Duration::hours(2);

    let game_id = Uuid::new_v4();
    
    // Insert game directly into database for testing
    sqlx::query!(
        r#"
        INSERT INTO games (id, home_team_id, away_team_id, season_id, week_number, game_start_time, status)
        VALUES ($1, $2, $3, $4, 1, $5, 'in_progress')
        "#,
        game_id,
        home_team_id,
        away_team_id,
        season_id,
        game_start
    )
    .execute(&test_app.db_pool)
    .await
    .expect("Failed to insert test game");

    game_id
}

async fn update_game_to_short_duration(test_app: &TestApp, game_id: Uuid) {
    let game_start = Utc::now();
    let game_end = game_start + Duration::minutes(1); // Very short game for testing
    
    sqlx::query!(
        r#"
        UPDATE games 
        SET game_start_time = $1, game_end_time = $2
        WHERE id = $3
        "#,
        game_start,
        game_end,
        game_id
    )
    .execute(&test_app.db_pool)
    .await
    .expect("Failed to update game duration");
}

async fn update_game_times_to_now(test_app: &TestApp, game_id: Uuid) {
    let now = Utc::now();
    let game_end = now + Duration::hours(2);
    
    sqlx::query!(
        r#"
        UPDATE games 
        SET game_start_time = $1, game_end_time = $2
        WHERE id = $3
        "#,
        now,
        game_end,
        game_id
    )
    .execute(&test_app.db_pool)
    .await
    .expect("Failed to update game times to current");
}

async fn start_test_game(test_app: &TestApp, game_id: Uuid) {
    let now = Utc::now();
    let game_end = now + Duration::hours(2);
    
    sqlx::query!(
        r#"
        UPDATE games 
        SET status = 'in_progress', 
            game_start_time = $1,
            game_end_time = $2
        WHERE id = $3
        "#,
        now,
        game_end,
        game_id
    )
    .execute(&test_app.db_pool)
    .await
    .expect("Failed to start test game");
}

async fn initialize_live_game(test_app: &TestApp, game_id: Uuid, _redis_client: Arc<redis::Client>) -> LiveGameRow {
    // In the consolidated architecture, games don't need separate initialization
    // Just start the game directly and return its state
    start_test_game(test_app, game_id).await;
    get_live_game_state(test_app, game_id).await
}

async fn get_season_id_for_game(test_app: &TestApp, game_id: Uuid) -> Uuid {
    let row = sqlx::query!(
        "SELECT season_id FROM games WHERE id = $1",
        game_id
    )
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to get season ID for game");
    
    row.season_id
}

async fn get_live_games_via_api(test_app: &TestApp, client: &Client, token: &str, season_id: Option<Uuid>) -> Vec<serde_json::Value> {
    let mut url = format!("{}/league/games/live-active", test_app.address);
    if let Some(sid) = season_id {
        url = format!("{}?season_id={}", url, sid);
    }

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get live games");

    assert!(response.status().is_success(), "Failed to get live games from API");
    
    let data: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(data["success"], true);
    
    data["data"].as_array().unwrap().clone()
}

async fn get_live_game_state(test_app: &TestApp, game_id: Uuid) -> LiveGameRow {
    // For tests that need detailed live game info, we still need to query the database
    // as the API endpoint returns league game data, not live game scoring data
    let row = sqlx::query!(
        r#"
        SELECT 
            g.id, g.id as game_id, g.home_team_id, ht.team_name as home_team_name, 
            g.away_team_id, at.team_name as away_team_name,
            g.home_score, g.away_score,
            g.game_start_time, g.game_end_time, g.last_score_time, g.last_scorer_id,
            g.last_scorer_name, g.last_scorer_team, g.created_at, g.updated_at
        FROM games g
        JOIN teams ht ON g.home_team_id = ht.id
        JOIN teams at ON g.away_team_id = at.id
        WHERE g.id = $1
        "#,
        game_id
    )
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to get live game state");

    LiveGameRow {
        id: row.id,
        game_id: row.game_id,
        home_team_name: row.home_team_name,
        away_team_name: row.away_team_name,
        home_score: row.home_score,
        away_score: row.away_score,
        game_start_time: row.game_start_time,
        game_end_time: row.game_end_time,
        last_score_time: row.last_score_time,
        last_scorer_id: row.last_scorer_id,
        last_scorer_name: row.last_scorer_name,
        last_scorer_team: row.last_scorer_team,
    }
}

async fn get_player_contributions(test_app: &TestApp, game_id: Uuid) -> (Vec<PlayerContribution>, Vec<PlayerContribution>) {
    // Get the game info first
    let game = sqlx::query!(
        "SELECT home_team_id, away_team_id FROM games WHERE id = $1",
        game_id
    )
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to get game info");

    // Get contributions by joining team_members with aggregated score events (same logic as the main system)
    let rows = sqlx::query!(
        r#"
        SELECT 
            tm.user_id,
            u.username,
            tm.team_id,
            t.team_name,
            CASE 
                WHEN tm.team_id = $2 THEN 'home'
                WHEN tm.team_id = $3 THEN 'away'
                ELSE 'unknown'
            END as team_side,
            COALESCE(SUM(lse.score_points), 0)::int as total_score_contribution,
            COUNT(CASE WHEN lse.id IS NOT NULL THEN 1 END)::int as contribution_count,
            MAX(lse.occurred_at) as last_contribution_time
        FROM team_members tm
        JOIN users u ON tm.user_id = u.id
        JOIN teams t ON tm.team_id = t.id
        LEFT JOIN live_score_events lse ON lse.game_id = $1 AND lse.user_id = tm.user_id
        WHERE tm.status = 'active'
        AND (tm.team_id = $2 OR tm.team_id = $3)
        GROUP BY tm.user_id, u.username, tm.team_id, t.team_name
        ORDER BY total_score_contribution DESC
        "#,
        game_id,
        game.home_team_id,
        game.away_team_id
    )
    .fetch_all(&test_app.db_pool)
    .await
    .expect("Failed to get player contributions");
    
    println!("DEBUG: Found {} team members for game_id: {}", rows.len(), game_id);
    println!("DEBUG: home_team_id: {}, away_team_id: {}", game.home_team_id, game.away_team_id);

    let mut home_contributions = Vec::new();
    let mut away_contributions = Vec::new();

    for row in rows {
        let team_side = row.team_side.as_ref().unwrap_or(&"unknown".to_string()).clone();
        let contrib = PlayerContribution {
            user_id: row.user_id,
            username: row.username,
            team_side: team_side.clone(),
            total_score_contribution: row.total_score_contribution.unwrap_or(0),
            contribution_count: row.contribution_count.unwrap_or(0),
            last_contribution_time: row.last_contribution_time,
        };

        if team_side == "home" {
            home_contributions.push(contrib);
        } else {
            away_contributions.push(contrib);
        }
    }

    (home_contributions, away_contributions)
}

async fn get_recent_score_events(test_app: &TestApp, game_id: Uuid) -> Vec<ScoreEvent> {
    let rows = sqlx::query!(
        r#"
        SELECT user_id, team_side, score_points, occurred_at
        FROM live_score_events 
        WHERE game_id = $1
        ORDER BY occurred_at DESC
        "#,
        game_id
    )
    .fetch_all(&test_app.db_pool)
    .await
    .expect("Failed to get score events");

    rows.into_iter().map(|row| ScoreEvent {
        user_id: row.user_id,
        team_side: row.team_side,
        score_points: row.score_points,
        occurred_at: row.occurred_at,
    }).collect()
}

    async fn finish_live_game(test_app: &TestApp, game_id: Uuid, _redis_client: Arc<redis::Client>) {
    // In the consolidated architecture, just update the game status to finished
    sqlx::query!(
        "UPDATE games SET status = 'finished' WHERE id = $1",
        game_id
    )
    .execute(&test_app.db_pool)
    .await
    .expect("Failed to finish test game");
}

#[tokio::test]
async fn test_live_game_workout_deletion_score_update() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let configuration = get_config().expect("Failed to read configuration.");
    let redis_client = Arc::new(redis::Client::open(RedisSettings::get_redis_url(&configuration.redis).expose_secret()).unwrap());

    // Setup test environment
    let live_game_environment = setup_live_game_environment(&test_app).await;
    
    // Get admin token for deletion
    let admin_session = create_admin_user_and_login(&test_app.address).await;
    
    // Update game times and start the game
    update_game_times_to_now(&test_app, live_game_environment.first_game_id).await;
    start_test_game(&test_app, live_game_environment.first_game_id).await;
    let live_game = initialize_live_game(&test_app, live_game_environment.first_game_id, redis_client).await;
    
    // Verify initial state
    assert_eq!(live_game.home_score, 0);
    assert_eq!(live_game.away_score, 0);
    
    println!("📊 Initial scores - Home: 0, Away: 0");
    
    // Step 1: Upload workouts and track their IDs
    // Home team workout
    let mut home_workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 30);
    let home_workout_response = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.home_user.token, &mut home_workout_data).await;
    assert!(home_workout_response.is_ok(), "Home workout upload should succeed");
    let home_workout_data: serde_json::Value = home_workout_response.unwrap();
    
    // Get workout ID from the health data response
    let home_workout_id = home_workout_data["data"]["sync_id"].as_str()
        .expect("Health upload response should contain sync_id");
    
    // Get the score gained from stat changes
    let home_score_gained = if let Some(game_stats) = home_workout_data["data"]["game_stats"].as_object() {
        game_stats["stamina_change"].as_i64().unwrap_or(0) + game_stats["strength_change"].as_i64().unwrap_or(0)
    } else {
        0
    };
    
    // Verify home team score increased
    let after_home_upload = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert!(after_home_upload.home_score > 0, "Home score should increase after workout");
    let home_score_before_deletion = after_home_upload.home_score;
    
    println!("📊 After home workout - Home: {}, Away: 0 (gained: {})", 
             home_score_before_deletion, home_score_gained);
    
    // Away team workouts
    let mut away1_workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);
    let away1_workout_response = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.away_user_1.token, &mut away1_workout_data).await;
    assert!(away1_workout_response.is_ok(), "Away workout upload should succeed");
    let away1_workout_data: serde_json::Value = away1_workout_response.unwrap();
    let away1_workout_id = away1_workout_data["data"]["sync_id"].as_str()
        .expect("Health upload response should contain sync_id");
    let away1_score_gained = if let Some(game_stats) = away1_workout_data["data"]["game_stats"].as_object() {
        game_stats["stamina_change"].as_i64().unwrap_or(0) + game_stats["strength_change"].as_i64().unwrap_or(0)
    } else {
        0
    };
    
    let mut away2_workout_data = WorkoutData::new(WorkoutType::Light, Utc::now(), 30);
    let away2_workout_response = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.away_user_2.token, &mut away2_workout_data).await;
    assert!(away2_workout_response.is_ok(), "Away workout upload should succeed");
    let away2_workout_data: serde_json::Value = away2_workout_response.unwrap();
    let away2_workout_id = away2_workout_data["data"]["sync_id"].as_str()
        .expect("Health upload response should contain sync_id");
    let away2_score_gained = if let Some(game_stats) = away2_workout_data["data"]["game_stats"].as_object() {
        game_stats["stamina_change"].as_i64().unwrap_or(0) + game_stats["strength_change"].as_i64().unwrap_or(0)
    } else {
        0
    };
    
    // Verify away team score increased
    let after_all_uploads = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert!(after_all_uploads.away_score > 0, "Away score should increase after workouts");
    let away_score_before_deletion = after_all_uploads.away_score;
    
    println!("📊 After all workouts - Home: {}, Away: {} (away gained: {} + {})", 
             after_all_uploads.home_score, away_score_before_deletion, 
             away1_score_gained, away2_score_gained);
    
    // Verify away team score increased
    let after_all_uploads = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert!(after_all_uploads.away_score > 0, "Away score should increase after workouts");
    let away_score_before_deletion = after_all_uploads.away_score;
    
    println!("📊 After all workouts - Home: {}, Away: {} (away gained: {} + {})", 
             after_all_uploads.home_score, away_score_before_deletion, 
             away1_score_gained, away2_score_gained);
    
    // Step 2: Delete home team workout
    let delete_response = make_authenticated_request(
        &client,
        reqwest::Method::DELETE,
        &format!("{}/admin/workouts/{}", test_app.address, home_workout_id),
        &admin_session.token,
        None,
    ).await;
    assert!(delete_response.status().is_success(), "Workout deletion should succeed");
    
    // Wait a bit for score recalculation
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Step 3: Verify home score decreased to 0
    let after_home_deletion = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert_eq!(after_home_deletion.home_score, 0, 
               "Home score should be 0 after deleting the only home workout");
    assert_eq!(after_home_deletion.away_score, away_score_before_deletion,
               "Away score should remain unchanged after home workout deletion");
    
    println!("📊 After home workout deletion - Home: 0, Away: {}", 
             after_home_deletion.away_score);
    
    // Step 4: Test bulk deletion of away team workouts
    let bulk_delete_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/workouts/bulk-delete", test_app.address),
        &admin_session.token,
        Some(json!({
            "workout_ids": [away1_workout_id, away2_workout_id]
        })),
    ).await;
    assert!(bulk_delete_response.status().is_success(), "Bulk deletion should succeed");
    
    // Wait a bit for score recalculation
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Step 5: Verify both scores are now 0
    let after_bulk_deletion = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert_eq!(after_bulk_deletion.home_score, 0, 
               "Home score should still be 0");
    assert_eq!(after_bulk_deletion.away_score, 0,
               "Away score should be 0 after deleting all away workouts");
    
    println!("📊 After bulk deletion - Home: 0, Away: 0");
    
    // Step 6: Upload new workout to verify system still works
    let mut workout_data = WorkoutData::new(WorkoutType::Light, Utc::now(), 30);
    let response = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.home_user.token, &mut workout_data).await;
    assert!(response.is_ok(), "Workout upload should succeed");
    
    let final_state = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert!(final_state.home_score > 0, 
            "Home score should increase after new workout post-deletion");
    assert_eq!(final_state.away_score, 0, 
               "Away score should remain 0");
    
    println!("📊 After new workout - Home: {}, Away: 0", final_state.home_score);
    
    // Step 7: Verify player contributions were updated correctly
    let (home_contributions, away_contributions) = get_player_contributions(&test_app, final_state.id).await;
    
    let home_user_contrib = home_contributions.iter()
        .find(|c| c.user_id == live_game_environment.home_user.user_id)
        .expect("Home user should have contribution");
    assert_eq!(home_user_contrib.contribution_count, 1, 
               "Home user should have 1 contribution after deletion and new upload");
    
    let away1_contrib = away_contributions.iter()
        .find(|c| c.user_id == live_game_environment.away_user_1.user_id)
        .expect("Away user 1 should have contribution record");
    assert_eq!(away1_contrib.total_score_contribution, 0,
               "Away user 1 should have 0 score contribution after deletion");
    assert_eq!(away1_contrib.contribution_count, 0,
               "Away user 1 should have 0 contribution count after deletion");
    
    println!("✅ Live game workout deletion test completed successfully!");
}

#[tokio::test]
async fn test_live_game_partial_workout_deletion() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let configuration = get_config().expect("Failed to read configuration.");
    let redis_client = Arc::new(redis::Client::open(RedisSettings::get_redis_url(&configuration.redis).expose_secret()).unwrap());

    // Setup test environment
    let live_game_environment = setup_live_game_environment(&test_app).await;
    let admin_session = create_admin_user_and_login(&test_app.address).await;
    
    // Start the game
    update_game_times_to_now(&test_app, live_game_environment.first_game_id).await;
    start_test_game(&test_app, live_game_environment.first_game_id).await;
    initialize_live_game(&test_app, live_game_environment.first_game_id, redis_client.clone()).await;
    
    // Upload multiple workouts for the same user (with different times to avoid duplicate detection)
    let mut workout_ids = Vec::new();
    let mut total_score = 0i64;
    
    for i in 0..3 {
        let workout_type = match i {
            0 => WorkoutType::Intense,
            1 => WorkoutType::Moderate,
            _ => WorkoutType::Light,
        };
        
        // Use different times for each workout (20 minutes apart) to avoid duplicate detection
        // but still within the 2-hour game window
        let workout_start = Utc::now() + Duration::minutes((i * 20) as i64);
        let mut workout_data = WorkoutData::new(workout_type, workout_start, 30);
        
        let response = upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.home_user.token, &mut workout_data).await;
        
        let response_data = response.unwrap();
        let workout_id = response_data["data"]["sync_id"].as_str()
            .expect("Health upload response should contain sync_id");
        workout_ids.push(workout_id.to_string());
        
        let score_gained = if let Some(game_stats) = response_data["data"]["game_stats"].as_object() {
            game_stats["stamina_change"].as_i64().unwrap_or(0) + game_stats["strength_change"].as_i64().unwrap_or(0)
        } else {
            0
        };
        total_score += score_gained;
        
        println!("📊 Workout {} uploaded, gained {} points", i + 1, score_gained);
    }
    
    // Verify total score
    let all_workouts_state = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    let full_score = all_workouts_state.home_score;
    assert!(full_score > 0, "Score should be positive after multiple workouts");
    println!("📊 Total score after 3 workouts: {}", full_score);
    
    // Delete the first workout
    let delete_response = make_authenticated_request(
        &client,
        reqwest::Method::DELETE,
        &format!("{}/admin/workouts/{}", test_app.address, workout_ids[0]),
        &admin_session.token,
        None,
    ).await;
    assert!(delete_response.status().is_success());
    
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Verify score decreased but not to 0
    let after_one_deletion = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert!(after_one_deletion.home_score > 0, 
            "Score should still be positive after deleting 1 of 3 workouts");
    assert!(after_one_deletion.home_score < full_score,
            "Score should decrease after workout deletion");
    
    println!("📊 Score after deleting 1 workout: {} (was {})", 
             after_one_deletion.home_score, full_score);
    
    // Verify player contribution count
    let (home_contributions, _) = get_player_contributions(&test_app, after_one_deletion.id).await;
    let home_user_contrib = home_contributions.iter()
        .find(|c| c.user_id == live_game_environment.home_user.user_id)
        .expect("Home user should have contribution");
    assert_eq!(home_user_contrib.contribution_count, 2,
               "Contribution count should be 2 after deleting 1 of 3 workouts");
    
    println!("✅ Partial workout deletion test completed successfully!");
}

#[tokio::test]
async fn test_user_joining_team_during_live_game() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let configuration = get_config().expect("Failed to read configuration.");
    let redis_client = Arc::new(redis::Client::open(RedisSettings::get_redis_url(&configuration.redis).expose_secret()).unwrap());

    // Setup live game environment
    let live_game_environment = setup_live_game_environment(&test_app).await;
    
    // Update game times to current and start the game
    update_game_times_to_now(&test_app, live_game_environment.first_game_id).await;
    start_test_game(&test_app, live_game_environment.first_game_id).await;
    let live_game = initialize_live_game(&test_app, live_game_environment.first_game_id, redis_client).await;
    
    // Verify initial state - only original team members should have contribution records
    let (initial_home_contributions, initial_away_contributions) = get_player_contributions(&test_app, live_game.id).await;
    
    let initial_home_count = initial_home_contributions.len();
    let initial_away_count = initial_away_contributions.len();
    
    println!("Initial contribution records - Home: {}, Away: {}", initial_home_count, initial_away_count);
    
    // Create a new user who will join the home team during the live game
    let new_user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &new_user).await.unwrap();
    println!("Created new user: {} ({})", new_user.username, new_user.user_id);
    
    // Verify the new user is NOT in the contributions before joining team
    let new_user_contrib_before = initial_home_contributions.iter()
        .find(|c| c.user_id == new_user.user_id);
    assert!(new_user_contrib_before.is_none(), "New user should not have contribution record before joining team");
    
    // Add the new user to the home team AFTER the live game has started
    add_user_to_team(&test_app.address, &live_game_environment.admin_session.token, &live_game_environment.home_team_id, new_user.user_id).await;
    println!("Added new user to home team during active live game");
    
    // Verify user is now a team member with zero contributions (new behavior with simplified system)
    let (contributions_after_join, _) = get_player_contributions(&test_app, live_game.id).await;
    let new_user_contrib_after_join = contributions_after_join.iter()
        .find(|c| c.user_id == new_user.user_id)
        .expect("New user should have contribution record with zero values after joining team");
    assert_eq!(new_user_contrib_after_join.total_score_contribution, 0, "New user should have zero contributions immediately after joining");
    assert_eq!(new_user_contrib_after_join.contribution_count, 0, "New user should have zero contribution count immediately after joining");
    
    // Now the new user uploads a workout during the active live game
    println!("About to upload workout for new user during live game...");
    println!("Live game info: id={}, start_time={}, end_time={}", 
        live_game.id, live_game.game_start_time.unwrap(), live_game.game_end_time.unwrap());
    
    // Use a workout time within the live game window (10 minutes after game start)
    let workout_start = live_game.game_start_time.unwrap() + Duration::minutes(10);
    println!("Using workout start time: {} (within game window)", workout_start);
    
    let mut workout_data = WorkoutData::new(WorkoutType::Moderate, workout_start, 30);
    let response = upload_workout_data_for_user(&client, &test_app.address, &new_user.token, &mut workout_data).await;
    
    assert!(response.is_ok(), "Workout upload should succeed");
    let response_data = response.unwrap();
    let stamina = response_data["data"]["game_stats"]["stat_changes"]["stamina_change"].as_i64().unwrap_or(0) as i32;
    let strength = response_data["data"]["game_stats"]["stat_changes"]["strength_change"].as_i64().unwrap_or(0) as i32;
    println!("New user uploaded workout: stamina={}, strength={}", stamina, strength);
    
    // Verify the live game score was updated
    let updated_live_game = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    println!("Updated live game state - home_score: {}, away_score: {}", 
        updated_live_game.home_score, updated_live_game.away_score);
    
    // Check what the initial score was
    println!("Initial live game state - home_score: {}, away_score: {}", 
        live_game.home_score, live_game.away_score);
        
    assert!(updated_live_game.home_score > live_game.home_score, "Home team score should have increased from new user's contribution");
    
    // Verify the new user now has a contribution record with actual contributions
    let (final_home_contributions, final_away_contributions) = get_player_contributions(&test_app, live_game.id).await;
    
    // Home team should have one more member than initially (the new user that joined)
    assert_eq!(final_home_contributions.len(), initial_home_count + 1, "Should have one additional home contribution record");
    assert_eq!(final_away_contributions.len(), initial_away_count, "Away contributions should remain unchanged");
    
    // Find the new user's contribution record
    let new_user_contrib = final_home_contributions.iter()
        .find(|c| c.user_id == new_user.user_id)
        .expect("New user should now have a contribution record");
        
    // Verify the contribution record was created correctly
    assert_eq!(new_user_contrib.username, new_user.username);
    assert_eq!(new_user_contrib.team_side, "home");
    assert_eq!(new_user_contrib.contribution_count, 1);
    assert!(new_user_contrib.total_score_contribution > 0, "Should have non-zero score contribution");
    assert!(new_user_contrib.is_recently_active(), "Should be marked as recently active");
    
    // Verify a score event was recorded for the new user
    let score_events = get_recent_score_events(&test_app, live_game.id).await;
    let new_user_event = score_events.iter()
        .find(|e| e.user_id == new_user.user_id)
        .expect("Should have score event for new user");
        
    assert_eq!(new_user_event.team_side, "home");
    assert!(new_user_event.score_points > 0.0, "Score event should have positive score");
    
    // Test that the new user can upload another workout and it continues to work
    let second_workout_start = live_game.game_start_time.unwrap() + Duration::minutes(45);
    let mut second_workout = WorkoutData::new(WorkoutType::Light, second_workout_start, 20);
    let response = upload_workout_data_for_user(&client, &test_app.address, &new_user.token, &mut second_workout).await;
    assert!(response.is_ok(), "Second workout should also succeed");
    
    // Verify the contribution count increased
    let (after_second_contributions, _) = get_player_contributions(&test_app, live_game.id).await;
    let new_user_final_contrib = after_second_contributions.iter()
        .find(|c| c.user_id == new_user.user_id)
        .expect("New user should still have contribution record");
        
    assert_eq!(new_user_final_contrib.contribution_count, 2, "Should have 2 contributions after second workout");
    assert!(new_user_final_contrib.total_score_contribution > new_user_contrib.total_score_contribution, 
        "Total score should have increased with second workout");
    
    println!("✅ User joining team during live game test completed successfully!");
    println!("New user {} successfully contributed to live game after joining team mid-game", new_user.username);

}

#[tokio::test]
async fn test_admin_live_game_score_adjustment() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let configuration = get_config().expect("Failed to read configuration.");
    let redis_client = Arc::new(redis::Client::open(RedisSettings::get_redis_url(&configuration.redis).expose_secret()).unwrap());

    // Setup test environment
    let live_game_environment = setup_live_game_environment(&test_app).await;
    let admin_session = create_admin_user_and_login(&test_app.address).await;
    
    // Start the game
    update_game_times_to_now(&test_app, live_game_environment.first_game_id).await;
    start_test_game(&test_app, live_game_environment.first_game_id).await;
    let live_game = initialize_live_game(&test_app, live_game_environment.first_game_id, redis_client).await;
    
    // Verify initial state
    assert_eq!(live_game.home_score, 0);
    assert_eq!(live_game.away_score, 0);
    
    println!("📊 Initial scores - Home: 0, Away: 0");

    // Upload a workout to give the home team some initial score
    let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 30);
    upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.home_user.token, &mut workout_data).await;
    
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    let after_workout = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    let initial_home_score = after_workout.home_score;
    
    assert!(initial_home_score > 0, "Home team should have score after workout");
    
    println!("📊 After workout - Home: {}", initial_home_score);

    // Test 1: Admin increases home team score
    let score_increase = 50;
    
    let adjust_request = json!({
        "game_id": live_game.id,
        "team_side": "home",
        "score_adjustment": score_increase,
        "reason": "Test admin increase"
    });

    let adjust_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/games/adjust-score", test_app.address),
        &admin_session.token,
        Some(adjust_request),
    ).await;

    assert!(adjust_response.status().is_success(), "Admin score adjustment should succeed");
    let response_data: serde_json::Value = adjust_response.json().await.unwrap();
    
    // Verify response structure
    assert_eq!(response_data["data"]["game_id"], live_game.id.to_string());
    assert_eq!(response_data["data"]["previous_scores"][0], initial_home_score);
    assert_eq!(response_data["data"]["previous_scores"][1], 0);
    assert_eq!(response_data["data"]["new_scores"][0], initial_home_score + score_increase);
    assert_eq!(response_data["data"]["new_scores"][1], 0);
    assert_eq!(response_data["data"]["adjustment_applied"], score_increase);
    
    println!("📊 After admin increase - Home: {} (was {})", 
             response_data["data"]["new_scores"][0], initial_home_score);

    // Verify actual database state
    let after_increase = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert_eq!(after_increase.home_score, initial_home_score + score_increase);
    assert_eq!(after_increase.away_score, 0);

    // Test 2: Admin decreases home team score
    let score_decrease = -30;
    
    let adjust_request = json!({
        "game_id": live_game.id,
        "team_side": "home",
        "score_adjustment": score_decrease,
        "reason": "Test admin decrease"
    });

    let adjust_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/games/adjust-score", test_app.address),
        &admin_session.token,
        Some(adjust_request),
    ).await;

    assert!(adjust_response.status().is_success(), "Admin score decrease should succeed");
    
    let after_decrease = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    let expected_score = initial_home_score + score_increase + score_decrease;
    
    assert_eq!(after_decrease.home_score, expected_score);
    
    println!("📊 After admin decrease - Home: {} (expected: {})", 
             after_decrease.home_score, expected_score);

    // Test 3: Admin adjusts away team score
    let away_score_increase = 75;
    
    let adjust_request = json!({
        "game_id": live_game.id,
        "team_side": "away",
        "score_adjustment": away_score_increase,
        "reason": "Test away team adjustment"
    });

    let adjust_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/games/adjust-score", test_app.address),
        &admin_session.token,
        Some(adjust_request),
    ).await;

    assert!(adjust_response.status().is_success(), "Away team adjustment should succeed");
    
    let after_away_adjust = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert_eq!(after_away_adjust.away_score, away_score_increase);
    assert_eq!(after_away_adjust.home_score, expected_score); // Should remain unchanged
    
    println!("📊 After away team adjustment - Home: {}, Away: {}", 
             after_away_adjust.home_score, after_away_adjust.away_score);

    // Test 4: Prevent negative scores
    let large_decrease = -1000;
    
    let adjust_request = json!({
        "game_id": live_game.id,
        "team_side": "away",
        "score_adjustment": large_decrease,
        "reason": "Test negative prevention"
    });

    let adjust_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/games/adjust-score", test_app.address),
        &admin_session.token,
        Some(adjust_request),
    ).await;

    assert!(adjust_response.status().is_success(), "Large decrease should succeed but be clamped");
    
    let after_clamp = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert_eq!(after_clamp.away_score, 0, "Score should be clamped to 0, not negative");
    
    println!("📊 After large decrease (clamped) - Away: {} (should be 0)", after_clamp.away_score);

    // Test 5: Invalid team side
    let invalid_request = json!({
        "game_id": live_game.id,
        "team_side": "middle",
        "score_adjustment": 10,
        "reason": "Test invalid team"
    });

    let invalid_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/games/adjust-score", test_app.address),
        &admin_session.token,
        Some(invalid_request),
    ).await;

    assert_eq!(invalid_response.status(), 400, "Invalid team side should return 400");

    // Test 6: Non-existent live game
    let nonexistent_request = json!({
        "game_id": "00000000-0000-0000-0000-000000000000",
        "team_side": "home",
        "score_adjustment": 10,
        "reason": "Test non-existent game"
    });

    let nonexistent_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/games/adjust-score", test_app.address),
        &admin_session.token,
        Some(nonexistent_request),
    ).await;

    assert_eq!(nonexistent_response.status(), 404, "Non-existent live game should return 404");

    println!("✅ Admin live game score adjustment test completed successfully!");
}

#[tokio::test]
async fn test_game_summary_creation_and_retrieval() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let configuration = get_config().expect("Failed to read configuration.");
    let redis_client = Arc::new(redis::Client::open(RedisSettings::get_redis_url(&configuration.redis).expose_secret()).unwrap());

    println!("📊 Testing Game Summary Creation and Retrieval");

    // Set up Redis subscription for WebSocket events
    let mut pubsub_conn = redis_client.get_async_connection().await.expect("Failed to create pubsub connection");
    let mut pubsub = pubsub_conn.into_pubsub();
    pubsub.subscribe("game:events:global").await.expect("Failed to subscribe to global channel");
    println!("✅ Subscribed to Redis global game events channel");

    // Step 1: Setup test environment - create league, season, teams, and users
    let live_game_environment = setup_live_game_environment(&test_app).await;

    // Update game times to current (games are auto-generated with future dates)
    update_game_times_to_now(&test_app, live_game_environment.first_game_id).await;

    // Start the game
    start_test_game(&test_app, live_game_environment.first_game_id).await;
    let live_game = initialize_live_game(&test_app, live_game_environment.first_game_id, redis_client.clone()).await;

    println!("✅ Game started: {}", live_game_environment.first_game_id);

    // Step 2: Upload workout data for players to generate scores
    // Home team: 1 player uploads intense workout
    let mut home_workout = WorkoutData::new(WorkoutType::Intense, Utc::now(), 45);
    upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.home_user.token, &mut home_workout).await.unwrap();

    // Away team: 2 players upload workouts with different intensities
    let mut away_workout1 = WorkoutData::new(WorkoutType::Intense, Utc::now(), 60);
    let mut away_workout2 = WorkoutData::new(WorkoutType::Light, Utc::now(), 20);
    upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.away_user_1.token, &mut away_workout1).await.unwrap();
    // No workout for user 2 on away team
    //upload_workout_data_for_user(&client, &test_app.address, &live_game_environment.away_user_2.token, &mut away_workout2).await.unwrap();

    println!("✅ Uploaded workouts for all players");

    // Step 3: Verify game has scores
    let updated_game = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert!(updated_game.home_score > 0, "Home team should have score");
    assert!(updated_game.away_score > 0, "Away team should have score");
    println!("📊 Game scores - Home: {}, Away: {}", updated_game.home_score, updated_game.away_score);

    // Step 4: Finish the game by setting status to 'finished'
    sqlx::query!(
        "UPDATE games SET game_end_time = NOW(), status = 'finished' WHERE id = $1",
        live_game_environment.first_game_id
    )
    .execute(&test_app.db_pool)
    .await
    .expect("Failed to finish game");
    println!("✅ Game finished");

    // Step 5: Get admin credentials and call the evaluation endpoint
    // This triggers game summary creation and broadcasts WebSocket events
    let admin_session = create_admin_user_and_login(&test_app.address).await;

    let today = Utc::now().format("%Y-%m-%d").to_string();
    let evaluation_request = json!({
        "date": today
    });

    let eval_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/games/evaluate", test_app.address),
        &admin_session.token,
        Some(evaluation_request),
    ).await;

    assert!(eval_response.status().is_success(), "Game evaluation should succeed");
    let eval_result = eval_response.json::<serde_json::Value>().await.unwrap();
    assert!(eval_result["success"].as_bool().unwrap_or(false), "Evaluation response should be successful");

    let games_evaluated = eval_result["games_evaluated"].as_u64().unwrap_or(0);
    assert!(games_evaluated > 0, "At least one game should have been evaluated");
    println!("✅ Game evaluated and summary created ({} games)", games_evaluated);

    // Step 5b: Listen for WebSocket events (game_summary_created)
    println!("🔍 Listening for WebSocket events...");
    use futures_util::StreamExt;
    use std::time::Duration;

    let mut game_summary_event_received = false;
    let mut global_stream = pubsub.on_message();
    let timeout = Duration::from_secs(10);
    let start_time = std::time::Instant::now();
    let target_game_id = live_game_environment.first_game_id.to_string();

    while start_time.elapsed() < timeout && !game_summary_event_received {
        tokio::select! {
            global_msg = global_stream.next() => {
                if let Some(msg) = global_msg {
                    if let Ok(payload) = msg.get_payload::<String>() {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload) {
                            let event_type = json["event_type"].as_str().unwrap_or("");

                            if event_type == "game_summary_created" {
                                let game_id = json["game_id"].as_str().unwrap_or("");
                                println!("📨 GameSummaryCreated event received for game: {}", game_id);

                                // Check if this is our target game
                                if game_id == target_game_id {
                                    game_summary_event_received = true;

                                    // Verify event structure
                                    assert!(json["summary_id"].as_str().is_some(), "Should have summary_id");
                                    assert!(json["mvp_username"].as_str().is_some(), "Should have MVP username");
                                    assert!(json["lvp_username"].as_str().is_some(), "Should have LVP username");
                                    assert!(json["final_home_score"].as_i64().is_some(), "Should have final_home_score");
                                    assert!(json["final_away_score"].as_i64().is_some(), "Should have final_away_score");

                                    let mvp = json["mvp_username"].as_str().unwrap();
                                    let lvp = json["lvp_username"].as_str().unwrap();
                                    let home_score = json["final_home_score"].as_i64().unwrap();
                                    let away_score = json["final_away_score"].as_i64().unwrap();
                                    println!("✅ GameSummaryCreated WebSocket event received for our game!");
                                    println!("   Score: {} - {}, MVP: {}, LVP: {}", home_score, away_score, mvp, lvp);
                                    break;
                                } else {
                                    println!("   (Skipping event for different game: {})", game_id);
                                }
                            }
                        }
                    }
                }
            },
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                // Continue loop
            }
        }
    }

    assert!(game_summary_event_received, "Should have received game_summary_created WebSocket event for game {}", target_game_id);

    // Step 6: Retrieve the game summary via API
    let summary_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/games/{}/summary", test_app.address, live_game_environment.first_game_id),
        &live_game_environment.home_user.token,
        None::<serde_json::Value>,
    ).await;

    assert!(summary_response.status().is_success(), "Game summary endpoint should succeed");

    let summary_data: serde_json::Value = summary_response.json().await.unwrap();
    println!("📊 Game summary response: {}", serde_json::to_string_pretty(&summary_data).unwrap());

    // Step 7: Validate game summary data
    assert!(summary_data["success"].as_bool().unwrap(), "Response should be successful");

    let summary = &summary_data["data"]["summary"];
    let home_team_name = summary_data["data"]["home_team_name"].as_str().unwrap();
    let away_team_name = summary_data["data"]["away_team_name"].as_str().unwrap();

    println!("🏟️  Teams: {} vs {}", home_team_name, away_team_name);

    // Verify game overview
    assert_eq!(summary["game_id"].as_str().unwrap(), live_game_environment.first_game_id.to_string());
    assert!(summary["final_home_score"].as_i64().is_some(), "Should have final home score");
    assert!(summary["final_away_score"].as_i64().is_some(), "Should have final away score");
    assert!(summary["game_start_date"].as_str().is_some(), "Should have game start date");
    assert!(summary["game_end_date"].as_str().is_some(), "Should have game end date");

    // Verify MVP exists (most valuable player)
    assert!(summary["mvp_user_id"].as_str().is_some(), "Should have MVP");
    assert!(summary["mvp_username"].as_str().is_some(), "Should have MVP username");
    assert_eq!(summary["mvp_username"].as_str().unwrap(), live_game_environment.away_user_1.username);
    assert!(summary["mvp_score_contribution"].as_i64().is_some(), "Should have MVP score contribution");

    let mvp_username = summary["mvp_username"].as_str().unwrap();
    let mvp_score = summary["mvp_score_contribution"].as_i64().unwrap();
    println!("🏆 MVP: {} with {} points", mvp_username, mvp_score);

    // Verify LVP exists (least valuable player)
    assert!(summary["lvp_user_id"].as_str().is_some(), "Should have LVP");
    assert!(summary["lvp_username"].as_str().is_some(), "Should have LVP username");
    // LVP should be either away_user_2 or the away team owner
    let lvp_username = summary["lvp_username"].as_str().unwrap();
    
    // Get the away team owner's username from the database
    let away_team_owner_username = sqlx::query!(
        "SELECT u.username FROM teams t JOIN users u ON t.user_id = u.id WHERE t.id = $1",
        Uuid::parse_str(&live_game_environment.away_team_id).unwrap()
    )
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to get away team owner username")
    .username;

    let home_team_owner_username = sqlx::query!(
        "SELECT u.username FROM teams t JOIN users u ON t.user_id = u.id WHERE t.id = $1",
        Uuid::parse_str(&live_game_environment.home_team_id).unwrap()
    )
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to get home team owner username")
    .username;
    
    let expected_lvp_usernames = vec![
        live_game_environment.away_user_2.username,
        away_team_owner_username,
        home_team_owner_username,
    ];
    assert!(expected_lvp_usernames.contains(&lvp_username.to_string()), 
        "LVP should be one of: {:?}, but got: {}", expected_lvp_usernames, lvp_username);
    assert!(summary["lvp_score_contribution"].as_i64().is_some(), "Should have LVP score contribution");

    let lvp_username = summary["lvp_username"].as_str().unwrap();
    let lvp_score = summary["lvp_score_contribution"].as_i64().unwrap();
    println!("📉 LVP: {} with {} points", lvp_username, lvp_score);

    // CRITICAL: Verify MVP and LVP are different players
    assert_ne!(mvp_username, lvp_username, "MVP and LVP should be different players! MVP: {}, LVP: {}", mvp_username, lvp_username);
    println!("✅ MVP and LVP are correctly different players");

    // Verify home team statistics
    assert!(summary["home_team_avg_score_per_player"].as_f64().is_some(), "Should have home team avg score");
    assert_eq!(summary["home_team_total_workouts"].as_i64().unwrap(), 1, "Home team should have 1 workout");
    assert!(summary["home_team_top_scorer_username"].as_str().is_some(), "Should have home team top scorer");

    let home_avg = summary["home_team_avg_score_per_player"].as_f64().unwrap();
    let home_top_scorer = summary["home_team_top_scorer_username"].as_str().unwrap();
    println!("🏠 Home team - Avg: {:.2}, Top scorer: {}", home_avg, home_top_scorer);

    // Verify away team statistics
    assert!(summary["away_team_avg_score_per_player"].as_f64().is_some(), "Should have away team avg score");
    assert_eq!(summary["away_team_total_workouts"].as_i64().unwrap(), 1, "Away team should have 1 workout");
    assert!(summary["away_team_top_scorer_username"].as_str().is_some(), "Should have away team top scorer");
    assert!(summary["away_team_lowest_performer_username"].as_str().is_some(), "Should have away team lowest performer");

    let away_avg = summary["away_team_avg_score_per_player"].as_f64().unwrap();
    let away_top_scorer = summary["away_team_top_scorer_username"].as_str().unwrap();
    let away_lowest = summary["away_team_lowest_performer_username"].as_str().unwrap();
    println!("✈️  Away team - Avg: {:.2}, Top scorer: {}, Lowest: {}", away_avg, away_top_scorer, away_lowest);

    // Step 8: Verify evaluated game appears in recent results
    println!("🔍 Step 8: Verifying evaluated game appears in recent results...");
    let recent_results_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/games/results?season_id={}&limit=10", test_app.address, live_game_environment.season_id),
        &live_game_environment.home_user.token,
        None::<serde_json::Value>,
    ).await;

    assert_eq!(recent_results_response.status(), 200, "Recent results request should succeed");

    let recent_results_data: serde_json::Value = recent_results_response.json().await.unwrap();
    assert!(recent_results_data["success"].as_bool().unwrap(), "Recent results should be successful");

    let recent_games = recent_results_data["data"].as_array().unwrap();
    assert!(!recent_games.is_empty(), "Should have at least one recent result");

    // Debug: Print the first game to see its structure
    if !recent_games.is_empty() {
        println!("📋 First game structure: {}", serde_json::to_string_pretty(&recent_games[0]).unwrap());
    }

    // Verify our evaluated game is in the results
    let target_game_id = live_game_environment.first_game_id.to_string();
    let found_game = recent_games.iter().find(|g| {
        // Game data is nested under "game" key
        g["game"]["id"].as_str() == Some(target_game_id.as_str())
    });
    assert!(found_game.is_some(), "Evaluated game should appear in recent results");

    let found_game = found_game.unwrap();
    // Status is in PascalCase format (e.g., "Evaluated")
    assert_eq!(found_game["game"]["status"].as_str().unwrap(), "Evaluated", "Game status should be 'Evaluated'");
    println!("✅ Evaluated game found in recent results with status 'Evaluated'");

    // Step 9: Test error case - summary for non-existent game
    let non_existent_game_id = Uuid::new_v4();
    let error_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/games/{}/summary", test_app.address, non_existent_game_id),
        &live_game_environment.home_user.token,
        None::<serde_json::Value>,
    ).await;

    assert_eq!(error_response.status(), 404, "Non-existent game should return 404");

    let error_data: serde_json::Value = error_response.json().await.unwrap();
    assert!(!error_data["success"].as_bool().unwrap(), "Error response should have success=false");

    println!("✅ Game summary creation and retrieval test completed successfully!");
}