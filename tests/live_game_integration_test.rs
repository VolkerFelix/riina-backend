use reqwest::Client;
use serde_json::json;
use chrono::{Utc, Duration, Weekday, NaiveTime, DateTime};
use uuid::Uuid;

mod common;
use common::live_game_helpers::*;
use common::workout_data_helpers::{WorkoutData, WorkoutType};

// Helper function to create test user with health profile for heart rate zone calculations
async fn create_test_user_with_health_profile(test_app: &TestApp, client: &Client) -> UserRegLoginResponse {
    let user = create_test_user_and_login(&test_app.address).await;
    
    // Create health profile using API
    let health_profile_data = json!({
        "age": 25,
        "gender": "male",
        "resting_heart_rate": 60
    });
    
    let response = make_authenticated_request(
        client,
        reqwest::Method::PUT,
        &format!("{}/profile/health_profile", test_app.address),
        &user.token,
        Some(health_profile_data),
    ).await;
    
    assert!(response.status().is_success(), "Failed to create health profile");
    
    user
}
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request, TestApp, get_next_date, UserRegLoginResponse};
use common::admin_helpers::{create_admin_user_and_login, create_league_season, create_teams_for_test, create_league, add_team_to_league, add_user_to_team};


#[tokio::test]
async fn test_complete_live_game_workflow() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Step 1: Setup test environment - create league, season, teams, and users
    let live_game_environment: LiveGameEnvironmentResult = 
        setup_live_game_environment(&test_app).await;
    
    // Update game times to current (games are auto-generated with future dates)
    update_game_times_to_now(&test_app, live_game_environment.first_game_id).await;
    
    start_test_game(&test_app, live_game_environment.first_game_id).await;

    let live_game = initialize_live_game(&test_app, live_game_environment.first_game_id).await;
    
    // Verify initial live game state
    assert_eq!(live_game.home_score, 0);
    assert_eq!(live_game.away_score, 0);
    assert_eq!(live_game.home_power, 0);
    assert_eq!(live_game.away_power, 0);
    assert!(live_game.is_active);

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
    let (stamina, strength) = upload_workout_data(&test_app, &client, &live_game_environment.home_user, WorkoutType::Intense).await;
    println!("DEBUG: Home user workout generated stamina: {}, strength: {}", stamina, strength);
    
    // Wait for live game processing
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    // Verify live game was updated
    let updated_live_game = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    println!("DEBUG: Updated live game - home_score: {}, home_power: {}", updated_live_game.home_score, updated_live_game.home_power);
    
    assert!(updated_live_game.home_score > 0, "Home team score should increase after workout upload");
    assert!(updated_live_game.home_power > 0, "Home team power should increase");
    assert_eq!(updated_live_game.away_score, 0, "Away team score should remain 0");
    
    // Verify last scorer information
    assert_eq!(updated_live_game.last_scorer_id, Some(live_game_environment.home_user.user_id));
    assert_eq!(updated_live_game.last_scorer_name, Some(live_game_environment.home_user.username.clone()));
    assert_eq!(updated_live_game.last_scorer_team, Some("home".to_string()));

    // Away team users upload workout data
    upload_workout_data(&test_app, &client, &live_game_environment.away_user_1, WorkoutType::Intense).await;
    upload_workout_data(&test_app, &client, &live_game_environment.away_user_2, WorkoutType::Intense).await;
    
    // Wait for live game processing
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    // Verify live game reflects both team activities
    let final_live_game = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    
    assert!(final_live_game.home_score > 0, "Home team should have score");
    assert!(final_live_game.away_score > 0, "Away team should have score after uploads");
    assert!(final_live_game.away_power > 0, "Away team power should increase");
    
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

    // Step 9: Test multiple uploads from same user
    upload_workout_data(&test_app, &client, &live_game_environment.home_user, WorkoutType::Intense).await;
    
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
    
    // This should not crash or cause errors
    upload_workout_data(&test_app, &client, &test_user, WorkoutType::Light).await;

    // Test 2: Multiple initializations of same live game
    let live_game_environment: LiveGameEnvironmentResult = 
        setup_live_game_environment(&test_app).await;
    
    update_game_times_to_now(&test_app, live_game_environment.first_game_id).await;
    start_test_game(&test_app, live_game_environment.first_game_id).await;

    let live_game_1 = initialize_live_game(&test_app, live_game_environment.first_game_id).await;
    let live_game_2 = initialize_live_game(&test_app, live_game_environment.first_game_id).await;
    
    // Should return same live game, not create duplicate
    assert_eq!(live_game_1.id, live_game_2.id);

    println!("✅ Live game edge cases test completed successfully!");
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
        INSERT INTO league_games (id, season_id, home_team_id, away_team_id, scheduled_time, week_number, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        game_id,
        season_id,
        home_team_id,
        away_team_id,
        now,
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

    // Setup and create a game that ends soon
    let live_game_environment: LiveGameEnvironmentResult = 
        setup_live_game_environment(&test_app).await;
    
    // Update the auto-generated game to end in 1 minute for testing
    update_game_to_short_duration(&test_app, live_game_environment.first_game_id).await;
    start_test_game(&test_app, live_game_environment.first_game_id).await;
    
    let live_game = initialize_live_game(&test_app, live_game_environment.first_game_id).await;
    
    // Upload some data while game is active
    upload_workout_data(&test_app, &client, &live_game_environment.home_user, WorkoutType::Intense).await;
    
    let active_game = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert!(active_game.is_active);
    assert!(active_game.home_score > 0);

    // Wait for game to end (in real test, we'd manipulate time or end the game programmatically)
    finish_live_game(&test_app, live_game.id).await;
    
    let finished_game = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert!(!finished_game.is_active);
    
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
    let final_score = finished_game.home_score;
    upload_workout_data(&test_app, &client, &live_game_environment.home_user, WorkoutType::Intense).await;
    
    let post_finish_game = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert_eq!(post_finish_game.home_score, final_score, "Score should not change after game ends");

    println!("✅ Live game finish workflow test completed successfully!");
}

#[tokio::test]
async fn test_workout_timing_validation_for_live_games() {
    let test_app = spawn_app().await;
    let client = Client::new();

    println!("🧪 Testing workout timing validation for live games...");

    // Setup test environment
    let live_game_environment: LiveGameEnvironmentResult = setup_live_game_environment(&test_app).await;
    
    // Update game times for precise testing
    update_game_times_to_now(&test_app, live_game_environment.first_game_id).await;
    start_test_game(&test_app, live_game_environment.first_game_id).await;
    
    let live_game = initialize_live_game(&test_app, live_game_environment.first_game_id).await;
    let game_start = live_game.game_start_time;
    let game_end = live_game.game_end_time;
    
    println!("📅 Game window: {} to {}", game_start, game_end);
    
    // Test 1: Workout BEFORE game start - should NOT count
    println!("\n🔬 Test 1: Uploading workout from before game start...");
    let before_game_workout = WorkoutData::new(
        WorkoutType::Intense, 
        game_start - Duration::hours(2),
        30
    );
    
    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", test_app.address),
        &live_game_environment.home_user.token,
        Some(before_game_workout.to_json()),
    ).await;
    assert!(response.status().is_success(), "Workout upload should succeed");
    
    // Check that score didn't increase
    let game_state_1 = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert_eq!(game_state_1.home_score, 0, "Score should not increase for workout before game start");
    println!("✅ Workout before game start correctly ignored");
    
    // Test 2: Workout DURING game - should count
    println!("\n🔬 Test 2: Uploading workout during game window...");
    // Use current time (which should be during the game since the game just started)
    // instead of a custom historical timestamp
    let during_game_workout = WorkoutData::new(WorkoutType::Intense, Utc::now(), 30);
    
    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", test_app.address),
        &live_game_environment.home_user.token,
        Some(during_game_workout.to_json()),
    ).await;
    assert!(response.status().is_success(), "Workout upload should succeed");
    
    // Debug: Check the response to see if game_stats are present
    let response_data: serde_json::Value = response.json().await.unwrap();
    println!("🔍 During game workout response: {}", serde_json::to_string_pretty(&response_data).unwrap_or_default());
    
    // Wait for live game processing
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    
    // Check that score increased
    let game_state_2 = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    println!("🔍 Game state after intense workout of home team: home_score={}, away_score={}", game_state_2.home_score, game_state_2.away_score);
    
    if game_state_2.home_score == 0 {
        // Additional debug: Check if the workout has game_stats at all
        if response_data["data"]["game_stats"].is_null() {
            println!("❌ No game_stats in workout response - workout timing might not align with game window");
            println!("🔍 Game start: {}, Workout time: {}, Game end: {}", 
                     game_start, 
                     game_start + Duration::hours(1),
                     game_end);
        } else {
            println!("❌ Game stats present but live score not updated");
            println!("🔍 Game state: {}", serde_json::to_string_pretty(&game_state_2).unwrap_or_default());
        }
    }
    
    assert!(game_state_2.home_score > 0, "Score should increase for workout during game. Game state: home={}, away={}", 
            game_state_2.home_score, game_state_2.away_score);
    let score_during_game = game_state_2.home_score;
    println!("✅ Workout during game correctly counted: +{} points", score_during_game);
    
    // Test 3: Workout AFTER game end - should NOT count
    println!("\n🔬 Test 3: Uploading workout from after game end...");
    let after_game_workout = WorkoutData::new(
        WorkoutType::Intense, 
        game_end + Duration::hours(1),
        30
    );
    
    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", test_app.address),
        &live_game_environment.home_user.token,
        Some(after_game_workout.to_json()),
    ).await;
    assert!(response.status().is_success(), "Workout upload should succeed");
    
    // Check that score didn't increase further
    let game_state_3 = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert_eq!(game_state_3.home_score, score_during_game, "Score should not increase for workout after game end");
    println!("✅ Workout after game end correctly ignored");
    
    // Test 4: Workout exactly at game start - should count
    println!("\n🔬 Test 4: Uploading workout exactly at game start...");
    let at_start_workout = WorkoutData::new(
        WorkoutType::Intense, 
        game_start,
        30
    );
    
    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", test_app.address),
        &live_game_environment.away_user_1.token,
        Some(at_start_workout.to_json()),
    ).await;
    assert!(response.status().is_success(), "Workout upload should succeed");
    
    // Check that away team score increased
    let game_state_4 = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert!(game_state_4.away_score > 0, "Score should increase for workout at game start");
    println!("✅ Workout at game start correctly counted: +{} points", game_state_4.away_score);
    
    // Test 5: Workout exactly at game end - should count
    println!("\n🔬 Test 5: Uploading workout exactly at game end...");
    let at_end_workout = WorkoutData::new(
        WorkoutType::Intense, 
        game_end,
        30
    );
    
    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", test_app.address),
        &live_game_environment.away_user_1.token,
        Some(at_end_workout.to_json()),
    ).await;
    assert!(response.status().is_success(), "Workout upload should succeed");
    
    // Check that away team score increased
    let game_state_5 = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert!(game_state_5.away_score > game_state_4.away_score, "Score should increase for workout at game end");
    println!("✅ Workout at game end correctly counted: +{} points", game_state_5.away_score - game_state_4.away_score);
    
    println!("\n🎉 All workout timing validation tests passed!");
    println!("Final scores - Home: {}, Away: {}", game_state_5.home_score, game_state_5.away_score);
}

#[tokio::test]
async fn test_live_game_workout_deletion_score_update() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Setup test environment
    let live_game_environment: LiveGameEnvironmentResult = setup_live_game_environment(&test_app).await;
        
    // Update game times and start the game
    update_game_times_to_now(&test_app, live_game_environment.first_game_id).await;
    start_test_game(&test_app, live_game_environment.first_game_id).await;
    let live_game = initialize_live_game(&test_app, live_game_environment.first_game_id).await;
    
    // Verify initial state
    assert_eq!(live_game.home_score, 0);
    assert_eq!(live_game.away_score, 0);
    
    println!("📊 Initial scores - Home: 0, Away: 0");
    
    // Step 1: Upload workouts and track their IDs
    // Wait a moment to ensure live game is fully initialized
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Home team workout
    let home_workout_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", test_app.address),
        &live_game_environment.home_user.token,
        Some(WorkoutData::new(WorkoutType::Intense, Utc::now(), 30).to_json()),
    ).await;
    assert!(home_workout_response.status().is_success());
    let home_workout_data: serde_json::Value = home_workout_response.json().await.unwrap();
    
    // Get workout ID from the health data response
    let home_workout_id = home_workout_data["data"]["sync_id"].as_str()
        .expect("Health upload response should contain sync_id");
    
    // Debug: Print the full response to understand structure
    println!("🔍 Home workout response: {}", serde_json::to_string_pretty(&home_workout_data).unwrap_or_default());
    
    // Get the score gained from stat changes
    let home_score_gained = if let Some(stat_changes) = home_workout_data["data"]["game_stats"]["stat_changes"].as_object() {
        stat_changes["stamina_change"].as_i64().unwrap_or(0) + stat_changes["strength_change"].as_i64().unwrap_or(0)
    } else {
        0
    };
    
    // Wait a moment for live game processing
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    // Verify home team score increased
    let after_home_upload = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    println!("🔍 Live game state after home upload: home_score={}, away_score={}", after_home_upload.home_score, after_home_upload.away_score);
    
    if after_home_upload.home_score == 0 {
        // Debug: Check if the workout has game_stats at all
        if home_workout_data["data"]["game_stats"].is_null() {
            println!("❌ No game_stats in workout response - workout may not be during active game");
        } else {
            println!("❌ Game stats present but live score not updated");
        }
    }
    
    assert!(after_home_upload.home_score > 0, 
            "Home score should increase after workout. Got score: {}, gained: {}", 
            after_home_upload.home_score, home_score_gained);
    let home_score_before_deletion = after_home_upload.home_score;
    
    println!("📊 After home workout - Home: {}, Away: 0 (gained: {})", 
             home_score_before_deletion, home_score_gained);
    
    // Away team workouts
    let away1_workout_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", test_app.address),
        &live_game_environment.away_user_1.token,
        Some(WorkoutData::new(WorkoutType::Intense, Utc::now(), 30).to_json()),
    ).await;
    assert!(away1_workout_response.status().is_success());
    let away1_workout_data: serde_json::Value = away1_workout_response.json().await.unwrap();
    let away1_workout_id = away1_workout_data["data"]["sync_id"].as_str()
        .expect("Health upload response should contain sync_id");
    let away1_score_gained = if let Some(stat_changes) = away1_workout_data["data"]["game_stats"]["stat_changes"].as_object() {
        stat_changes["stamina_change"].as_i64().unwrap_or(0) + stat_changes["strength_change"].as_i64().unwrap_or(0)
    } else {
        0
    };
    
    let away2_workout_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", test_app.address),
        &live_game_environment.away_user_2.token,
        Some(WorkoutData::new(WorkoutType::Intense, Utc::now(), 30).to_json()),
    ).await;
    assert!(away2_workout_response.status().is_success());
    let away2_workout_data: serde_json::Value = away2_workout_response.json().await.unwrap();
    let away2_workout_id = away2_workout_data["data"]["sync_id"].as_str()
        .expect("Health upload response should contain sync_id");
    let away2_score_gained = if let Some(stat_changes) = away2_workout_data["data"]["game_stats"]["stat_changes"].as_object() {
        stat_changes["stamina_change"].as_i64().unwrap_or(0) + stat_changes["strength_change"].as_i64().unwrap_or(0)
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
    
    // Step 2: Delete home team workout
    let delete_response = make_authenticated_request(
        &client,
        reqwest::Method::DELETE,
        &format!("{}/admin/workouts/{}", test_app.address, home_workout_id),
        &live_game_environment.admin_session.token,
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
        &live_game_environment.admin_session.token,
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
    upload_workout_data(&test_app, &client, &live_game_environment.home_user, WorkoutType::Light).await;
    
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

    // Setup test environment
    let live_game_environment: LiveGameEnvironmentResult = setup_live_game_environment(&test_app).await;
    let admin_session = create_admin_user_and_login(&test_app.address).await;
    
    // Start the game
    update_game_times_to_now(&test_app, live_game_environment.first_game_id).await;
    start_test_game(&test_app, live_game_environment.first_game_id).await;
    initialize_live_game(&test_app, live_game_environment.first_game_id).await;
    
    // Upload multiple workouts for the same user
    let mut workout_ids = Vec::new();
    let mut total_score = 0i64;
    
    for i in 0..3 {
        let workout_type = match i {
            0 => WorkoutType::Intense,
            1 => WorkoutType::Moderate,
            _ => WorkoutType::Light,
        };
        
        let response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/health/upload_health", test_app.address),
            &live_game_environment.home_user.token,
            Some(WorkoutData::new(workout_type, Utc::now(), 30).to_json()),
        ).await;
        
        let workout_data: serde_json::Value = response.json().await.unwrap();
        let workout_id = workout_data["data"]["sync_id"].as_str()
            .expect("Health upload response should contain sync_id");
        workout_ids.push(workout_id.to_string());
        
        let score_gained = if let Some(stat_changes) = workout_data["data"]["game_stats"]["stat_changes"].as_object() {
            stat_changes["stamina_change"].as_i64().unwrap_or(0) + stat_changes["strength_change"].as_i64().unwrap_or(0)
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
async fn test_admin_live_game_score_adjustment() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Setup test environment
    let live_game_environment: LiveGameEnvironmentResult = setup_live_game_environment(&test_app).await;
    let admin_session = create_admin_user_and_login(&test_app.address).await;
    
    // Start the game
    update_game_times_to_now(&test_app, live_game_environment.first_game_id).await;
    start_test_game(&test_app, live_game_environment.first_game_id).await;
    let live_game = initialize_live_game(&test_app, live_game_environment.first_game_id).await;
    
    // Verify initial state
    assert_eq!(live_game.home_score, 0);
    assert_eq!(live_game.away_score, 0);
    assert_eq!(live_game.home_power, 0);
    assert_eq!(live_game.away_power, 0);
    
    println!("📊 Initial scores - Home: 0, Away: 0");

    // Upload a workout to give the home team some initial score
    upload_workout_data(&test_app, &client, &live_game_environment.home_user, WorkoutType::Intense).await;
    
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    let after_workout = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    let initial_home_score = after_workout.home_score;
    let initial_home_power = after_workout.home_power;
    
    assert!(initial_home_score > 0, "Home team should have score after workout");
    assert!(initial_home_power > 0, "Home team should have power after workout");
    
    println!("📊 After workout - Home: {} (power: {}), Away: 0", initial_home_score, initial_home_power);

    // Test 1: Admin increases home team score
    let score_increase = 50;
    let power_increase = 25;
    
    let adjust_request = json!({
        "live_game_id": live_game.id,
        "team_side": "home",
        "score_adjustment": score_increase,
        "power_adjustment": power_increase,
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
    assert_eq!(response_data["data"]["live_game_id"], live_game.id.to_string());
    assert_eq!(response_data["data"]["previous_scores"][0], initial_home_score);
    assert_eq!(response_data["data"]["previous_scores"][1], 0);
    assert_eq!(response_data["data"]["new_scores"][0], initial_home_score + score_increase);
    assert_eq!(response_data["data"]["new_scores"][1], 0);
    assert_eq!(response_data["data"]["adjustment_applied"][0], score_increase);
    assert_eq!(response_data["data"]["adjustment_applied"][1], power_increase);
    
    println!("📊 After admin increase - Home: {} (was {}), Power: {} (was {})", 
             response_data["data"]["new_scores"][0], initial_home_score,
             response_data["data"]["new_power"][0], initial_home_power);

    // Verify actual database state
    let after_increase = get_live_game_state(&test_app, live_game_environment.first_game_id).await;
    assert_eq!(after_increase.home_score, initial_home_score + score_increase);
    assert_eq!(after_increase.home_power, initial_home_power + power_increase);
    assert_eq!(after_increase.away_score, 0);
    assert_eq!(after_increase.away_power, 0);

    // Test 2: Admin decreases home team score
    let score_decrease = -30;
    let power_decrease = -10;
    
    let adjust_request = json!({
        "live_game_id": live_game.id,
        "team_side": "home",
        "score_adjustment": score_decrease,
        "power_adjustment": power_decrease,
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
    let expected_power = initial_home_power + power_increase + power_decrease;
    
    assert_eq!(after_decrease.home_score, expected_score);
    assert_eq!(after_decrease.home_power, expected_power);
    
    println!("📊 After admin decrease - Home: {} (expected: {}), Power: {} (expected: {})", 
             after_decrease.home_score, expected_score,
             after_decrease.home_power, expected_power);

    // Test 3: Admin adjusts away team score
    let away_score_increase = 75;
    let away_power_increase = 40;
    
    let adjust_request = json!({
        "live_game_id": live_game.id,
        "team_side": "away",
        "score_adjustment": away_score_increase,
        "power_adjustment": away_power_increase,
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
    assert_eq!(after_away_adjust.away_power, away_power_increase);
    assert_eq!(after_away_adjust.home_score, expected_score); // Should remain unchanged
    assert_eq!(after_away_adjust.home_power, expected_power); // Should remain unchanged
    
    println!("📊 After away team adjustment - Home: {}, Away: {} (power: {})", 
             after_away_adjust.home_score, after_away_adjust.away_score, after_away_adjust.away_power);

    // Test 4: Prevent negative scores
    let large_decrease = -1000;
    
    let adjust_request = json!({
        "live_game_id": live_game.id,
        "team_side": "away",
        "score_adjustment": large_decrease,
        "power_adjustment": large_decrease,
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
    assert_eq!(after_clamp.away_power, 0, "Power should be clamped to 0, not negative");
    
    println!("📊 After large decrease (clamped) - Away: {} (should be 0)", after_clamp.away_score);

    // Test 5: Invalid team side
    let invalid_request = json!({
        "live_game_id": live_game.id,
        "team_side": "middle",
        "score_adjustment": 10,
        "power_adjustment": 10,
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
        "live_game_id": "00000000-0000-0000-0000-000000000000",
        "team_side": "home",
        "score_adjustment": 10,
        "power_adjustment": 10,
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