use reqwest::Client;
use serde_json::json;
use chrono::{Utc, Duration, Weekday, NaiveTime, DateTime};
use uuid::Uuid;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request, TestApp, get_next_date, UserRegLoginResponse};
use common::admin_helpers::{create_admin_user_and_login, create_league_season, create_teams_for_test, create_league, add_team_to_league, add_user_to_team};


#[tokio::test]
async fn test_complete_live_game_workflow() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Step 1: Setup test environment - create league, season, teams, and users
    let (home_user, away_user_1, away_user_2, game_id) = 
        setup_live_game_environment(&test_app).await;
    
    // Update game times to current (games are auto-generated with future dates)
    update_game_times_to_now(&test_app, game_id).await;
    
    start_test_game(&test_app, game_id).await;

    let live_game = initialize_live_game(&test_app, game_id).await;
    
    // Verify initial live game state
    assert_eq!(live_game.home_score, 0);
    assert_eq!(live_game.away_score, 0);
    assert_eq!(live_game.home_power, 0);
    assert_eq!(live_game.away_power, 0);
    assert!(live_game.is_active);

    // Step 4: Verify the game appears in the live games API endpoint
    // Get season ID for the API call
    let season_id = get_season_id_for_game(&test_app, game_id).await;
    
    // Fetch live games via API
    let live_games = get_live_games_via_api(&test_app, &client, &home_user.token, Some(season_id)).await;
    
    // Verify our game is in the live games list
    assert!(!live_games.is_empty(), "Should have at least one live game");
    
    let our_game = live_games.iter().find(|g| g["game"]["id"].as_str() == Some(&game_id.to_string()));
    assert!(our_game.is_some(), "Our game should be in the live games list");
    
    let api_game = our_game.unwrap();
    assert_eq!(api_game["game"]["status"].as_str(), Some("InProgress"));
    assert!(api_game["home_team_name"].is_string(), "Should have home team name");
    assert!(api_game["away_team_name"].is_string(), "Should have away team name");

    // Step 5: Test score updates through health data uploads
    // Home team user uploads workout data
    upload_workout_data(&test_app, &client, &home_user, "intense_workout").await;
    // Verify live game was updated
    let updated_live_game = get_live_game_state(&test_app, game_id).await;
    
    assert!(updated_live_game.home_score > 0, "Home team score should increase after workout upload");
    assert!(updated_live_game.home_power > 0, "Home team power should increase");
    assert_eq!(updated_live_game.away_score, 0, "Away team score should remain 0");
    
    // Verify last scorer information
    assert_eq!(updated_live_game.last_scorer_id, Some(home_user.user_id));
    assert_eq!(updated_live_game.last_scorer_name, Some(home_user.username.clone()));
    assert_eq!(updated_live_game.last_scorer_team, Some("home".to_string()));

    // Away team users upload workout data
    upload_workout_data(&test_app, &client, &away_user_1, "moderate_workout").await;
    upload_workout_data(&test_app, &client, &away_user_2, "light_workout").await;
    
    // Verify live game reflects both team activities
    let final_live_game = get_live_game_state(&test_app, game_id).await;
    
    assert!(final_live_game.home_score > 0, "Home team should have score");
    assert!(final_live_game.away_score > 0, "Away team should have score after uploads");
    assert!(final_live_game.away_power > 0, "Away team power should increase");
    
    // For now, just check that both teams have scores
    assert!(final_live_game.home_score > 0, "Home team should have score");
    assert!(final_live_game.away_score > 0, "Away team should have score after uploads");

    // Step 6: Test player contributions tracking
    let (home_contributions, away_contributions) = get_player_contributions(&test_app, final_live_game.id).await;
    
    // Verify home team contribution - filter to only the user we're testing
    let home_user_contrib = home_contributions.iter()
        .find(|c| c.user_id == home_user.user_id)
        .expect("Home user should have a contribution record");
    assert!(home_user_contrib.total_score_contribution > 0);
    assert_eq!(home_user_contrib.contribution_count, 1);
    assert!(home_user_contrib.is_recently_active());

    // Verify away team contributions - filter to only users with actual contributions
    let away_active_contributions: Vec<&PlayerContribution> = away_contributions.iter()
        .filter(|c| c.total_score_contribution > 0)
        .collect();
    assert_eq!(away_active_contributions.len(), 1, "Only away_user_1 should have non-zero contribution");
    assert!(away_active_contributions.iter().any(|c| c.user_id == away_user_1.user_id));

    // Step 7: Test score events logging
    let score_events = get_recent_score_events(&test_app, final_live_game.id).await;
    assert_eq!(score_events.len(), 2, "Should have 2 score events (only for users with > 0 stats)");
    
    // Verify events are properly logged
    assert!(score_events.iter().any(|e| e.user_id == home_user.user_id && e.team_side == "home"));
    assert!(score_events.iter().any(|e| e.user_id == away_user_1.user_id && e.team_side == "away"));
    // Away user 2 should not have a score event since their workout generated 0 points

    // Step 8: Test game progress and time calculations
    assert!(final_live_game.game_progress() >= 0.0 && final_live_game.game_progress() <= 100.0);
    assert!(final_live_game.time_remaining().is_some());

    // Step 9: Test multiple uploads from same user
    upload_workout_data(&test_app, &client, &home_user, "second_workout").await;
    
    let after_second_upload = get_live_game_state(&test_app, game_id).await;
    assert!(after_second_upload.home_score > final_live_game.home_score, 
        "Score should increase after second workout");

    // Verify contribution count increased
    let (updated_home_contributions, _) = get_player_contributions(&test_app, after_second_upload.id).await;
    let home_contrib = updated_home_contributions.iter()
        .find(|c| c.user_id == home_user.user_id)
        .expect("Home user should have contributions");
    assert_eq!(home_contrib.contribution_count, 2, "Home user should have 2 contributions after second upload");

    // Step 10: Test live scoring history API endpoint
    test_live_scoring_history_api(&test_app, &client, &home_user.token, game_id, &home_user, &away_user_1).await;

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
    upload_workout_data(&test_app, &client, &test_user, "no_game_workout").await;

    // Test 2: Multiple initializations of same live game
    let (_, _, _, game_id) = 
        setup_live_game_environment(&test_app).await;
    
    update_game_times_to_now(&test_app, game_id).await;
    start_test_game(&test_app, game_id).await;

    let live_game_1 = initialize_live_game(&test_app, game_id).await;
    let live_game_2 = initialize_live_game(&test_app, game_id).await;
    
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
    let (home_user, _, _, game_id) = 
        setup_live_game_environment(&test_app).await;
    
    // Update the auto-generated game to end in 1 minute for testing
    update_game_to_short_duration(&test_app, game_id).await;
    start_test_game(&test_app, game_id).await;
    
    let live_game = initialize_live_game(&test_app, game_id).await;
    
    // Upload some data while game is active
    upload_workout_data(&test_app, &client, &home_user, "last_minute_workout").await;
    
    let active_game = get_live_game_state(&test_app, game_id).await;
    assert!(active_game.is_active);
    assert!(active_game.home_score > 0);

    // Wait for game to end (in real test, we'd manipulate time or end the game programmatically)
    finish_live_game(&test_app, live_game.id).await;
    
    let finished_game = get_live_game_state(&test_app, game_id).await;
    assert!(!finished_game.is_active);
    
    // Verify the game no longer appears in the live games API
    let season_id = get_season_id_for_game(&test_app, game_id).await;
    let live_games_after_finish = get_live_games_via_api(&test_app, &client, &home_user.token, Some(season_id)).await;
    
    // The finished game should NOT appear in the live games list
    let finished_game_in_api = live_games_after_finish.iter().find(|g| g["game"]["id"].as_str() == Some(&game_id.to_string()));
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
    upload_workout_data(&test_app, &client, &home_user, "after_game_workout").await;
    
    let post_finish_game = get_live_game_state(&test_app, game_id).await;
    assert_eq!(post_finish_game.home_score, final_score, "Score should not change after game ends");

    println!("✅ Live game finish workflow test completed successfully!");
}

async fn test_live_scoring_history_api(
    test_app: &TestApp, 
    client: &Client, 
    token: &str, 
    game_id: Uuid,
    home_user: &UserRegLoginResponse,
    away_user: &UserRegLoginResponse
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
        let score_points = event_obj["score_points"].as_i64().expect("score_points should be a number");
        assert!(score_points > 0, "Test uploads should generate positive points");
        
        // Verify the user_id matches one of our test users
        let event_user_id = event_obj["user_id"].as_str().expect("user_id should be a string");
        let event_user_uuid = Uuid::parse_str(event_user_id).expect("user_id should be valid UUID");
        assert!(
            event_user_uuid == home_user.user_id || event_user_uuid == away_user.user_id,
            "Event should be from one of our test users"
        );
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

// Helper functions

async fn setup_live_game_environment(
    test_app: &TestApp, 
) -> (UserRegLoginResponse, UserRegLoginResponse, UserRegLoginResponse, Uuid) {
    let admin_session = create_admin_user_and_login(&test_app.address).await;
    // Create league
    let league_id = create_league(&test_app.address, &admin_session.token, 2).await;

    // Create teams
    let team_ids = create_teams_for_test(&test_app.address, &admin_session.token, 2).await;
    let home_team_id = team_ids[0].clone();
    let away_team_id = team_ids[1].clone();

    // Create additional users and add them to teams
    let home_user = create_test_user_and_login(&test_app.address).await;
    let away_user_1 = create_test_user_and_login(&test_app.address).await;
    let away_user_2 = create_test_user_and_login(&test_app.address).await;

    // Add users to teams
    add_user_to_team(&test_app.address, &admin_session.token, &home_team_id, home_user.user_id).await;
    add_user_to_team(&test_app.address, &admin_session.token, &away_team_id, away_user_1.user_id).await;
    add_user_to_team(&test_app.address, &admin_session.token, &away_team_id, away_user_2.user_id).await;

    // Add teams to league BEFORE creating the season so games are auto-generated
    add_team_to_league(&test_app.address, &admin_session.token, &league_id, &home_team_id).await;
    add_team_to_league(&test_app.address, &admin_session.token, &league_id, &away_team_id).await;

    let start_date = get_next_date(Weekday::Sat, NaiveTime::from_hms_opt(22, 0, 0).unwrap());
    let season_id = create_league_season(&test_app.address, &admin_session.token, &league_id, "Test Season", &start_date.to_rfc3339()).await;
    
    // Get the auto-generated game ID
    let game_id = get_first_game_for_teams(&test_app, Uuid::parse_str(&season_id).unwrap(), Uuid::parse_str(&home_team_id).unwrap(), Uuid::parse_str(&away_team_id).unwrap()).await;

    (home_user, away_user_1, away_user_2, game_id)
}

async fn get_first_game_for_teams(test_app: &TestApp, season_id: Uuid, home_team_id: Uuid, away_team_id: Uuid) -> Uuid {
    // Get the auto-generated game between these teams
    let game = sqlx::query!(
        r#"
        SELECT id 
        FROM league_games 
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
        INSERT INTO league_games (id, home_team_id, away_team_id, season_id, week_number, scheduled_time, status, week_start_date, week_end_date)
        VALUES ($1, $2, $3, $4, 1, $5, 'in_progress', $6, $7)
        "#,
        game_id,
        home_team_id,
        away_team_id,
        season_id,
        game_start,
        game_start,
        game_end
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
        UPDATE league_games 
        SET week_start_date = $1, week_end_date = $2, scheduled_time = $1
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
        UPDATE league_games 
        SET scheduled_time = $1, week_start_date = $1, week_end_date = $2
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
    sqlx::query!(
        "UPDATE league_games SET status = 'in_progress' WHERE id = $1",
        game_id
    )
    .execute(&test_app.db_pool)
    .await
    .expect("Failed to start test game");
}

async fn initialize_live_game(test_app: &TestApp, game_id: Uuid) -> LiveGameRow {
    // Create live game service and initialize
    let live_game_service = evolveme_backend::services::LiveGameService::new(test_app.db_pool.clone(), None);
    
    live_game_service.initialize_live_game(game_id)
        .await
        .expect("Failed to initialize live game");

    get_live_game_state(test_app, game_id).await
}

async fn get_season_id_for_game(test_app: &TestApp, game_id: Uuid) -> Uuid {
    let row = sqlx::query!(
        "SELECT season_id FROM league_games WHERE id = $1",
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
            id, game_id, home_team_id, home_team_name, away_team_id, away_team_name,
            home_score, away_score, home_power, away_power,
            game_start_time, game_end_time, last_score_time, last_scorer_id,
            last_scorer_name, last_scorer_team, is_active, created_at, updated_at
        FROM live_games 
        WHERE game_id = $1
        ORDER BY created_at DESC
        LIMIT 1
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
        home_power: row.home_power,
        away_power: row.away_power,
        game_start_time: row.game_start_time,
        game_end_time: row.game_end_time,
        last_score_time: row.last_score_time,
        last_scorer_id: row.last_scorer_id,
        last_scorer_name: row.last_scorer_name,
        last_scorer_team: row.last_scorer_team,
        is_active: row.is_active,
    }
}

async fn upload_workout_data(
    test_app: &TestApp, 
    client: &Client, 
    user: &common::utils::UserRegLoginResponse, 
    workout_type: &str
) -> (i32, i32) {
    let heart_rate_data = match workout_type {
        "intense_workout" => generate_intense_workout_data(),
        "moderate_workout" => generate_moderate_workout_data(),
        "light_workout" => generate_light_workout_data(),
        "second_workout" => generate_moderate_workout_data(),
        "last_minute_workout" => generate_intense_workout_data(),
        "after_game_workout" => generate_light_workout_data(),
        "no_game_workout" => generate_light_workout_data(),
        _ => generate_light_workout_data(),
    };

    let health_data = json!({
        "device_id": format!("test-device-{}", user.username),
        "timestamp": Utc::now(),
        "heart_rate": heart_rate_data,
        "active_energy_burned": Option::<f64>::None
    });

    let response = make_authenticated_request(
        client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", test_app.address),
        &user.token,
        Some(health_data),
    ).await;

    assert!(response.status().is_success(), "Health data upload should succeed");
    
    // Return actual calculated values based on the response
    let response_data: serde_json::Value = response.json().await.unwrap();
    if let Some(game_stats) = response_data["data"]["game_stats"].as_object() {
        if let Some(stat_changes) = game_stats["stat_changes"].as_object() {
            let stamina = stat_changes["stamina_change"].as_i64().unwrap_or(0) as i32;
            let strength = stat_changes["strength_change"].as_i64().unwrap_or(0) as i32;
            return (stamina, strength);
        }
    }
    
    // Fallback - this shouldn't happen if the response is successful
    panic!("Failed to extract stat changes from response: {:?}", response_data);
}

async fn get_player_contributions(test_app: &TestApp, live_game_id: Uuid) -> (Vec<PlayerContribution>, Vec<PlayerContribution>) {
    let rows = sqlx::query!(
        r#"
        SELECT 
            user_id, username, team_side, current_power, 
            total_score_contribution, contribution_count, last_contribution_time
        FROM live_player_contributions 
        WHERE live_game_id = $1
        ORDER BY total_score_contribution DESC
        "#,
        live_game_id
    )
    .fetch_all(&test_app.db_pool)
    .await
    .expect("Failed to get player contributions");

    let mut home_contributions = Vec::new();
    let mut away_contributions = Vec::new();

    for row in rows {
        let contrib = PlayerContribution {
            user_id: row.user_id,
            username: row.username,
            team_side: row.team_side,
            total_score_contribution: row.total_score_contribution,
            contribution_count: row.contribution_count,
            last_contribution_time: row.last_contribution_time,
        };

        if contrib.team_side == "home" {
            home_contributions.push(contrib);
        } else {
            away_contributions.push(contrib);
        }
    }

    (home_contributions, away_contributions)
}

async fn get_recent_score_events(test_app: &TestApp, live_game_id: Uuid) -> Vec<ScoreEvent> {
    let rows = sqlx::query!(
        r#"
        SELECT user_id, team_side, score_points, occurred_at
        FROM live_score_events 
        WHERE live_game_id = $1
        ORDER BY occurred_at DESC
        "#,
        live_game_id
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

async fn finish_live_game(test_app: &TestApp, live_game_id: Uuid) {
    // Use the actual backend service instead of direct database calls
    let live_game_service = evolveme_backend::services::LiveGameService::new(test_app.db_pool.clone(), None);
    
    live_game_service.finish_live_game(live_game_id)
        .await
        .expect("Failed to finish live game using service");
}

// Helper functions for generating workout data
fn generate_intense_workout_data() -> Vec<serde_json::Value> {
    let base_time = Utc::now();
    (0..300).map(|i| json!({
        "timestamp": base_time + Duration::seconds(i * 2),
        "heart_rate": 140 + (i % 30) // High intensity heart rate
    })).collect()
}

fn generate_moderate_workout_data() -> Vec<serde_json::Value> {
    let base_time = Utc::now();
    (0..200).map(|i| json!({
        "timestamp": base_time + Duration::seconds(i * 3),
        "heart_rate": 110 + (i % 20) // Moderate intensity
    })).collect()
}

fn generate_light_workout_data() -> Vec<serde_json::Value> {
    let base_time = Utc::now();
    (0..100).map(|i| json!({
        "timestamp": base_time + Duration::seconds(i * 5),
        "heart_rate": 90 + (i % 15) // Light intensity
    })).collect()
}

// Test data structures
#[derive(Debug)]
struct LiveGameRow {
    id: Uuid,
    game_id: Uuid,
    home_team_name: String,
    away_team_name: String,
    home_score: i32,
    away_score: i32,
    home_power: i32,
    away_power: i32,
    game_start_time: DateTime<Utc>,
    game_end_time: DateTime<Utc>,
    last_score_time: Option<DateTime<Utc>>,
    last_scorer_id: Option<Uuid>,
    last_scorer_name: Option<String>,
    last_scorer_team: Option<String>,
    is_active: bool,
}

impl LiveGameRow {
    /// Calculate game progress as percentage (0-100)
    pub fn game_progress(&self) -> f32 {
        let now = Utc::now();
        if now < self.game_start_time {
            return 0.0;
        }
        if now >= self.game_end_time {
            return 100.0;
        }
        
        let total_duration = (self.game_end_time - self.game_start_time).num_milliseconds() as f32;
        let elapsed = (now - self.game_start_time).num_milliseconds() as f32;
        
        (elapsed / total_duration * 100.0).clamp(0.0, 100.0)
    }

    /// Get time remaining in human readable format
    pub fn time_remaining(&self) -> Option<String> {
        let now = Utc::now();
        if now >= self.game_end_time || !self.is_active {
            return Some("Final".to_string());
        }

        let remaining = self.game_end_time - now;
        let hours = remaining.num_hours();
        let minutes = remaining.num_minutes() % 60;

        if hours > 0 {
            Some(format!("{}h {}m", hours, minutes))
        } else if minutes > 0 {
            Some(format!("{}m", minutes))
        } else {
            Some("< 1m".to_string())
        }
    }
}

#[derive(Debug)]
struct PlayerContribution {
    user_id: Uuid,
    username: String,
    team_side: String,
    total_score_contribution: i32,
    contribution_count: i32,
    last_contribution_time: Option<chrono::DateTime<Utc>>,
}

impl PlayerContribution {
    fn is_recently_active(&self) -> bool {
        if let Some(last_contribution) = self.last_contribution_time {
            let thirty_minutes_ago = Utc::now() - Duration::minutes(30);
            last_contribution > thirty_minutes_ago
        } else {
            false
        }
    }
}

#[derive(Debug)]
struct ScoreEvent {
    user_id: Uuid,
    team_side: String,
    score_points: i32,
    occurred_at: chrono::DateTime<Utc>,
}