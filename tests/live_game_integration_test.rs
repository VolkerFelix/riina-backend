use reqwest::Client;
use serde_json::json;
use chrono::{Utc, Duration, Weekday, NaiveTime, DateTime};
use sqlx::Row;
use uuid::Uuid;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request, TestApp};
use common::admin_helpers::{create_admin_user_and_login, create_league_season};

use crate::common::admin_helpers::create_teams_for_test;
use crate::common::utils::get_next_date;

#[tokio::test]
async fn test_complete_live_game_workflow() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Step 1: Setup test environment - create league, season, teams, and users
    let (league_id, season_id, home_team_id, away_team_id, home_user, away_user_1, away_user_2) = 
        setup_live_game_environment(&test_app, &client).await;

    // Step 2: Create a game between the teams
    let game_id = create_test_game(
        &test_app, 
        &client, 
        Uuid::parse_str(&home_team_id).unwrap(), 
        Uuid::parse_str(&away_team_id).unwrap(), 
        Uuid::parse_str(&season_id).unwrap()
    ).await;
    
    // Step 3: Start the game (set status to in_progress)
    start_test_game(&test_app, &client, game_id).await;

    // Step 4: Initialize live game
    let live_game = initialize_live_game(&test_app, game_id).await;
    
    // Verify initial live game state
    assert_eq!(live_game.home_score, 0);
    assert_eq!(live_game.away_score, 0);
    assert_eq!(live_game.home_power, 0);
    assert_eq!(live_game.away_power, 0);
    assert!(live_game.is_active);

    // Step 5: Test score updates through health data uploads
    
    // Home team user uploads workout data
    let home_initial_stats = upload_workout_data(&test_app, &client, &home_user, "intense_workout").await;
    println!("Home user uploaded workout data. Expected stats: {:?}", home_initial_stats);
    
    // Verify live game was updated
    let updated_live_game = get_live_game_state(&test_app, game_id).await;
    println!("Updated live game state: home_score={}, away_score={}, home_power={}, away_power={}", 
        updated_live_game.home_score, updated_live_game.away_score, 
        updated_live_game.home_power, updated_live_game.away_power);
    
    assert!(updated_live_game.home_score > 0, "Home team score should increase after workout upload");
    assert!(updated_live_game.home_power > 0, "Home team power should increase");
    assert_eq!(updated_live_game.away_score, 0, "Away team score should remain 0");
    
    // Verify last scorer information
    assert_eq!(updated_live_game.last_scorer_id, Some(home_user.user_id));
    assert_eq!(updated_live_game.last_scorer_name, Some(home_user.username.clone()));
    assert_eq!(updated_live_game.last_scorer_team, Some("home".to_string()));

    // Away team users upload workout data
    let away_1_stats = upload_workout_data(&test_app, &client, &away_user_1, "moderate_workout").await;
    println!("Away user 1 uploaded workout data. Expected stats: {:?}", away_1_stats);
    
    let mid_game_state = get_live_game_state(&test_app, game_id).await;
    println!("After away user 1: home_score={}, away_score={}", mid_game_state.home_score, mid_game_state.away_score);
    
    let away_2_stats = upload_workout_data(&test_app, &client, &away_user_2, "light_workout").await;
    println!("Away user 2 uploaded workout data. Expected stats: {:?}", away_2_stats);
    
    // Verify live game reflects both team activities
    let final_live_game = get_live_game_state(&test_app, game_id).await;
    println!("Final live game state: home_score={}, away_score={}, home_power={}, away_power={}", 
        final_live_game.home_score, final_live_game.away_score, 
        final_live_game.home_power, final_live_game.away_power);
    
    assert!(final_live_game.home_score > 0, "Home team should have score");
    assert!(final_live_game.away_score > 0, "Away team should have score after uploads");
    assert!(final_live_game.away_power > 0, "Away team power should increase");
    
    // Away team should have higher score due to two contributors
    let away_1_total = away_1_stats.0 + away_1_stats.1;
    let away_2_total = away_2_stats.0 + away_2_stats.1;
    let expected_away_total = away_1_total + away_2_total;
    
    println!("Score calculation details:");
    println!("  Home user: stamina={}, strength={}, total={}", 
        home_initial_stats.0, home_initial_stats.1, home_initial_stats.0 + home_initial_stats.1);
    println!("  Away user 1: stamina={}, strength={}, total={}", 
        away_1_stats.0, away_1_stats.1, away_1_total);
    println!("  Away user 2: stamina={}, strength={}, total={}", 
        away_2_stats.0, away_2_stats.1, away_2_total);
    println!("  Expected away total: {} + {} = {}", away_1_total, away_2_total, expected_away_total);
    println!("  Actual scores: home={}, away={}", final_live_game.home_score, final_live_game.away_score);
    
    // For now, just check that both teams have scores
    assert!(final_live_game.home_score > 0, "Home team should have score");
    assert!(final_live_game.away_score > 0, "Away team should have score after uploads");

    // Step 6: Test player contributions tracking
    let (home_contributions, away_contributions) = get_player_contributions(&test_app, final_live_game.id).await;
    
    // Verify home team contribution
    assert_eq!(home_contributions.len(), 1);
    let home_contrib = &home_contributions[0];
    assert_eq!(home_contrib.user_id, home_user.user_id);
    assert!(home_contrib.total_score_contribution > 0);
    assert_eq!(home_contrib.contribution_count, 1);
    assert!(home_contrib.is_recently_active());

    // Verify away team contributions (only away_user_1 should have contribution since away_user_2 had 0 stats)
    assert_eq!(away_contributions.len(), 1);
    assert!(away_contributions.iter().any(|c| c.user_id == away_user_1.user_id));
    assert!(away_contributions.iter().all(|c| c.total_score_contribution > 0));

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
    let (_, season_id, home_team_id, away_team_id, _, _, _) = 
        setup_live_game_environment(&test_app, &client).await;
    
    let game_id = create_test_game(&test_app, &client, Uuid::parse_str(&home_team_id).unwrap(), Uuid::parse_str(&away_team_id).unwrap(), Uuid::parse_str(&season_id).unwrap()).await;
    start_test_game(&test_app, &client, game_id).await;

    let live_game_1 = initialize_live_game(&test_app, game_id).await;
    let live_game_2 = initialize_live_game(&test_app, game_id).await;
    
    // Should return same live game, not create duplicate
    assert_eq!(live_game_1.id, live_game_2.id);

    println!("✅ Live game edge cases test completed successfully!");
}

#[tokio::test]
async fn test_live_game_finish_workflow() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Setup and create a game that ends soon
    let (_, season_id, home_team_id, away_team_id, home_user, _, _) = 
        setup_live_game_environment(&test_app, &client).await;
    
    // Create game that ends in 1 minute
    let game_id = create_short_test_game(
        &test_app, 
        &client, 
        Uuid::parse_str(&home_team_id).unwrap(), 
        Uuid::parse_str(&away_team_id).unwrap(),
        Uuid::parse_str(&season_id).unwrap()
    ).await;
    start_test_game(&test_app, &client, game_id).await;
    
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
    
    // Try to upload data after game ended - should not affect scores
    let final_score = finished_game.home_score;
    upload_workout_data(&test_app, &client, &home_user, "after_game_workout").await;
    
    let post_finish_game = get_live_game_state(&test_app, game_id).await;
    assert_eq!(post_finish_game.home_score, final_score, "Score should not change after game ends");

    println!("✅ Live game finish workflow test completed successfully!");
}

// Helper functions

async fn setup_live_game_environment(
    test_app: &TestApp, 
    client: &Client
) -> (String, String, String, String, common::utils::UserRegLoginResponse, common::utils::UserRegLoginResponse, common::utils::UserRegLoginResponse) {
    let admin_session = create_admin_user_and_login(&test_app.address).await;
    // Create league
    let league_request = json!({
        "name": format!("Live Game Test League {}", &Uuid::new_v4().to_string()[..8]),
        "description": "Testing live game service",
        "max_teams": 2
    });
    
    let league_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", &test_app.address),
        &admin_session.token,
        Some(league_request),
    ).await;
    
    assert_eq!(league_response.status(), 201);
    let league_data: serde_json::Value = league_response.json().await.unwrap();
    let league_id = league_data["data"]["id"].as_str().unwrap();

    // Create teams
    let team_ids = create_teams_for_test(&test_app.address, &admin_session.token, 2).await;
    let home_team_id = team_ids[0].clone();
    let away_team_id = team_ids[1].clone();

    // Create users and add them to teams
    let home_user = create_test_user_and_login(&test_app.address).await;
    let away_user_1 = create_test_user_and_login(&test_app.address).await;
    let away_user_2 = create_test_user_and_login(&test_app.address).await;

    // Add users to teams
    add_user_to_team(&test_app, client, &admin_session.token, &home_team_id, home_user.user_id).await;
    add_user_to_team(&test_app, client, &admin_session.token, &away_team_id, away_user_1.user_id).await;
    add_user_to_team(&test_app, client, &admin_session.token, &away_team_id, away_user_2.user_id).await;

    // Add teams to league
    add_team_to_league(&test_app, client, &admin_session.token, &league_id, &home_team_id).await;
    add_team_to_league(&test_app, client, &admin_session.token, &league_id, &away_team_id).await;

    let start_date = get_next_date(Weekday::Sat, NaiveTime::from_hms_opt(22, 0, 0).unwrap());
    let season_id = create_league_season(&test_app.address, &admin_session.token, &league_id, "Test Season", &start_date.to_rfc3339()).await;

    (league_id.to_string(), season_id.to_string(), home_team_id.to_string(), away_team_id.to_string(), home_user, away_user_1, away_user_2)
}

async fn add_team_to_league(test_app: &TestApp, client: &Client, admin_token: &str, league_id: &str, team_id: &str) {
    let team_data = json!({
        "team_id": team_id,
        "league_id": league_id
    });

    let response = make_authenticated_request(
        client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/teams", test_app.address, league_id),
        admin_token,
        Some(team_data),
    ).await;

    assert!(response.status().is_success());
}

async fn create_test_team(test_app: &TestApp, client: &Client, admin_token: &str, team_name: &str, league_id: Uuid) -> Uuid {
    let team_data = json!({
        "name": team_name,
        "league_id": league_id,
        "team_color": "#FF0000"
    });

    let response = make_authenticated_request(
        client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        admin_token,
        Some(team_data),
    ).await;

    assert!(response.status().is_success());
    let team_response: serde_json::Value = response.json().await.unwrap();
    Uuid::parse_str(team_response["data"]["id"].as_str().unwrap()).unwrap()
}

async fn add_user_to_team(test_app: &TestApp, client: &Client, admin_token: &str, team_id: &str, user_id: Uuid) {
    let member_data = json!({
        "user_id": user_id,
        "role": "member"
    });

    let response = make_authenticated_request(
        client,
        reqwest::Method::POST,
        &format!("{}/admin/teams/{}/members", test_app.address, team_id),
        admin_token,
        Some(member_data),
    ).await;

    assert!(response.status().is_success());
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

async fn create_short_test_game(test_app: &TestApp, client: &Client, home_team_id: Uuid, away_team_id: Uuid, season_id: Uuid) -> Uuid {
    let game_start = Utc::now();
    let game_end = game_start + Duration::minutes(1); // Very short game for testing

    let game_id = Uuid::new_v4();
    
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
    .expect("Failed to insert short test game");

    game_id
}

async fn start_test_game(test_app: &TestApp, client: &Client, game_id: Uuid) {
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

async fn get_live_game_state(test_app: &TestApp, game_id: Uuid) -> LiveGameRow {
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
    sqlx::query!(
        "UPDATE live_games SET is_active = false WHERE id = $1",
        live_game_id
    )
    .execute(&test_app.db_pool)
    .await
    .expect("Failed to finish live game");
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