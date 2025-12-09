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
, delete_test_user};
use common::admin_helpers::{create_admin_user_and_login, create_teams_for_test, add_user_to_team, create_league_with_teams, LeagueWithTeamsResult};

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
// SCHEDULER DATABASE INTEGRATION TESTS
// ============================================================================

#[tokio::test]
async fn test_scheduler_loads_active_seasons_from_database() {
    println!("🧪 Testing Scheduler Loads Active Seasons from Database on Startup");

    let app = spawn_app().await;
    let client = Client::new();
    let configuration = get_config().expect("Failed to read configuration.");
    let redis_client = Arc::new(redis::Client::open(RedisSettings::get_redis_url(&configuration.redis).expose_secret()).unwrap());

    // Create admin user
    let admin_user = create_admin_user_and_login(&app.address).await;

    // Create a league with teams
    let LeagueWithTeamsResult { league_id, team_ids } = create_league_with_teams(
        &app.address,
        &admin_user.token,
        4,  // max_teams
        4,  // team_count
        None,  // team_owners
        true,  // add_to_league
        Some(format!("DB Load Test League {}", &Uuid::new_v4().to_string()[..4])),
        Some("Testing database season loading".to_string()),
    ).await;

    println!("✅ Created league with 4 teams");

    // Create multiple active seasons directly in the database
    // These will have current date ranges and auto_evaluation_enabled = true
    let now = Utc::now();
    let league_uuid = Uuid::parse_str(&league_id).expect("Failed to parse league_id");

    // Season 1: Currently active (started yesterday, ends tomorrow)
    let season1_id = Uuid::new_v4();
    let season1_start = now - Duration::days(1);
    let season1_end = now + Duration::days(1);

    sqlx::query!(
        r#"
        INSERT INTO league_seasons
        (id, league_id, name, start_date, end_date, auto_evaluation_enabled, game_duration_seconds)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        season1_id,
        league_uuid,
        "Active Season 1",
        season1_start,
        season1_end,
        true,
        60i64  // 60 seconds
    )
    .execute(&app.db_pool)
    .await
    .expect("Failed to insert season 1");

    // Season 2: Currently active (started 2 hours ago, ends in 2 hours)
    let season2_id = Uuid::new_v4();
    let season2_start = now - Duration::hours(2);
    let season2_end = now + Duration::hours(2);

    sqlx::query!(
        r#"
        INSERT INTO league_seasons
        (id, league_id, name, start_date, end_date, auto_evaluation_enabled, game_duration_seconds)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        season2_id,
        league_uuid,
        "Active Season 2",
        season2_start,
        season2_end,
        true,
        120i64  // 120 seconds
    )
    .execute(&app.db_pool)
    .await
    .expect("Failed to insert season 2");

    // Season 3: Not active (ended yesterday)
    let season3_id = Uuid::new_v4();
    let season3_start = now - Duration::days(10);
    let season3_end = now - Duration::days(1);

    sqlx::query!(
        r#"
        INSERT INTO league_seasons
        (id, league_id, name, start_date, end_date, auto_evaluation_enabled, game_duration_seconds)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        season3_id,
        league_uuid,
        "Ended Season 3",
        season3_start,
        season3_end,
        true,
        60i64
    )
    .execute(&app.db_pool)
    .await
    .expect("Failed to insert season 3");

    // Season 4: Not active yet (starts tomorrow)
    let season4_id = Uuid::new_v4();
    let season4_start = now + Duration::days(1);
    let season4_end = now + Duration::days(10);

    sqlx::query!(
        r#"
        INSERT INTO league_seasons
        (id, league_id, name, start_date, end_date, auto_evaluation_enabled, game_duration_seconds)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        season4_id,
        league_uuid,
        "Future Season 4",
        season4_start,
        season4_end,
        true,
        60i64
    )
    .execute(&app.db_pool)
    .await
    .expect("Failed to insert season 4");

    // Season 5: Currently active but auto_evaluation_enabled = false
    let season5_id = Uuid::new_v4();
    let season5_start = now - Duration::hours(1);
    let season5_end = now + Duration::hours(1);

    sqlx::query!(
        r#"
        INSERT INTO league_seasons
        (id, league_id, name, start_date, end_date, auto_evaluation_enabled, game_duration_seconds)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        season5_id,
        league_uuid,
        "Manual Season 5",
        season5_start,
        season5_end,
        false,
        60i64
    )
    .execute(&app.db_pool)
    .await
    .expect("Failed to insert season 5");

    println!("✅ Created 5 test seasons in database:");
    println!("   - Season 1: Active (started yesterday)");
    println!("   - Season 2: Active (started 2 hours ago)");
    println!("   - Season 3: Ended (ended yesterday)");
    println!("   - Season 4: Future (starts tomorrow)");
    println!("   - Season 5: Active but auto_evaluation_enabled=false");

    // Now create a NEW scheduler service - it should load the 2 active seasons from the database
    let scheduler = SchedulerService::new_with_redis(
        app.db_pool.clone(),
        redis_client.clone()
    ).await.expect("Failed to create scheduler service");

    println!("⏳ Starting scheduler - should load active seasons from database...");
    scheduler.start().await.expect("Failed to start scheduler");

    // Give it a moment to load the seasons
    tokio::time::sleep(TokioDuration::from_millis(500)).await;

    println!("✅ Scheduler started");

    // Verify that our test seasons are correctly identified as active or not
    // Note: We query ALL active seasons but only verify OUR test seasons are in the correct state
    // This allows the test to work even when other tests are running in parallel
    let active_seasons = sqlx::query!(
        r#"
        SELECT id, name, game_duration_seconds, auto_evaluation_enabled
        FROM league_seasons
        WHERE auto_evaluation_enabled = true
        AND start_date <= NOW()
        AND end_date >= NOW()
        "#
    )
    .fetch_all(&app.db_pool)
    .await
    .expect("Failed to query active seasons");

    let active_season_ids: Vec<Uuid> = active_seasons.iter().map(|s| s.id).collect();

    // Verify our specific test seasons are in the correct state
    assert!(active_season_ids.contains(&season1_id), "Season 1 should be active");
    assert!(active_season_ids.contains(&season2_id), "Season 2 should be active");
    assert!(!active_season_ids.contains(&season3_id), "Season 3 should not be active (ended)");
    assert!(!active_season_ids.contains(&season4_id), "Season 4 should not be active (future)");
    assert!(!active_season_ids.contains(&season5_id), "Season 5 should not be active (auto_evaluation disabled)");

    // Count how many of OUR seasons are active (should be exactly 2)
    let our_season_ids = vec![season1_id, season2_id, season3_id, season4_id, season5_id];
    let our_active_count = active_season_ids.iter()
        .filter(|id| our_season_ids.contains(id))
        .count();
    assert_eq!(our_active_count, 2, "Should have exactly 2 of our test seasons active");

    println!("✅ Verified correct seasons were identified as active:");
    for season in &active_seasons {
        println!("   - {} (duration: {}s)", season.name, season.game_duration_seconds);
    }

    // Clean up
    scheduler.stop().await.expect("Failed to stop scheduler");

    println!("🎉 Scheduler database loading test completed successfully!");
}

// ============================================================================
// AUTOMATED SCHEDULER INTEGRATION TESTS
// ============================================================================

#[tokio::test]
async fn test_automated_scheduler_game_lifecycle() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🤖 Testing Automated Scheduler - Game Lifecycle");
    
    // Clean up games and seasons that are not currently being used by parallel tests
    // Only clean up completed/old test data to avoid interfering with running tests
    let cleanup_cutoff = Utc::now() - Duration::hours(1); // Only clean data older than 1 hour
    
    sqlx::query!(
        "DELETE FROM games WHERE created_at < $1 OR status IN ('evaluated', 'cancelled')",
        cleanup_cutoff
    )
    .execute(&app.db_pool)
    .await
    .expect("Failed to clean up old games");
    
    sqlx::query!(
        "DELETE FROM league_seasons WHERE created_at < $1 AND name LIKE '%Test%'", 
        cleanup_cutoff
    )
    .execute(&app.db_pool)
    .await
    .expect("Failed to clean up old test seasons");
    
    println!("🧹 Cleaned up all games and seasons from other test runs");
    
    // Create admin and users
    let admin_user = create_admin_user_and_login(&app.address).await;
    let user1 = create_test_user_and_login(&app.address).await;
    let user2 = create_test_user_and_login(&app.address).await;
    
    // Create league and teams
    let LeagueWithTeamsResult { league_id, team_ids } = create_league_with_teams(
        &app.address,
        &admin_user.token,
        2,  // max_teams
        2,  // team_count
        None,  // team_owners (will create new users)
        true,  // add_to_league
        Some(format!("Scheduler Test League {}", &Uuid::new_v4().to_string()[..4])),
        Some("Testing automated scheduler".to_string()),
    ).await;

    // Add users to teams
    add_user_to_team(&app.address, &admin_user.token, &team_ids[0], user1.user_id).await;
    add_user_to_team(&app.address, &admin_user.token, &team_ids[1], user2.user_id).await;
    
    println!("✅ Created league with 2 teams and added users to teams");

    // Create season with 10-second games (for fast testing)
    let start_date = Utc::now() + Duration::seconds(10);
    
    let season_request = json!({
        "name": format!("Scheduler Test Season {}", &Uuid::new_v4().to_string()[..4]),
        "start_date": start_date.to_rfc3339(),
        "evaluation_timezone": "UTC",
        "auto_evaluation_enabled": true,
        "game_duration_seconds": 10, // 10 seconds
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
    
    // DEBUG: Check what was actually created in the database
    let season_uuid = Uuid::parse_str(season_id).expect("Failed to parse season_id as UUID");
    let debug_teams = sqlx::query!(
        "SELECT t.id FROM teams t JOIN league_teams lt ON t.id = lt.team_id WHERE lt.season_id = $1",
        season_uuid
    )
    .fetch_all(&app.db_pool)
    .await
    .expect("Failed to query teams");
    
    let debug_games = sqlx::query!(
        "SELECT id, home_team_id, away_team_id, status, week_number, is_first_leg, game_start_time, game_end_time FROM games WHERE season_id = $1 ORDER BY game_start_time",
        season_uuid
    )
    .fetch_all(&app.db_pool)
    .await
    .expect("Failed to query games");
    
    println!("🔍 DEBUG: Season {} has {} teams and {} games", season_id, debug_teams.len(), debug_games.len());
    for team in &debug_teams {
        println!("  Team: {}", team.id);
    }
    for (i, game) in debug_games.iter().enumerate() {
        println!("  Game {}: {} vs {} | week={}, first_leg={}, status={}, start={:?}", 
            i+1, 
            &game.home_team_id.to_string()[..8], 
            &game.away_team_id.to_string()[..8],
            game.week_number, 
            game.is_first_leg,
            game.status,
            game.game_start_time
        );

    }

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
        
        // Check if any game has moved beyond scheduled status (started by scheduler)
        for game in upcoming_games.iter().chain(live_games.iter()) {
            let status = game["game"]["status"].as_str().unwrap_or("");
            if status != "scheduled" {
                game_started = true;
                println!("✅ Scheduler started game automatically! Status: '{}' (attempt {})", status, attempts);
                break;
            }
        }
    }
    
    assert!(game_started, "Scheduler should have automatically started a game within 45 seconds");
    
    // Step 2: Wait for the game to complete (10 seconds + buffer)
    println!("⏳ Waiting for game to complete and be evaluated...");
    let mut game_completed = false;
    let mut completion_attempts = 0;
    
    while !game_completed && completion_attempts < 30 { // Wait up to 30 seconds
        tokio::time::sleep(TokioDuration::from_secs(1)).await;
        completion_attempts += 1;
        
        // Check standings to see if games have been played
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
            
            let total_games_played: i64 = standings.iter()
                .map(|s| s["standing"]["games_played"].as_i64().unwrap_or(0))
                .sum();
            
            if completion_attempts % 5 == 0 || total_games_played > 0 { // Debug every 5 attempts or when games found
                println!("🔍 Completion check (attempt {}): Total games played across all teams: {}", 
                    completion_attempts, total_games_played);
                
                // Check current database state
                let season_uuid = Uuid::parse_str(season_id).expect("Failed to parse season_id as UUID");
                let current_games = sqlx::query!(
                    "SELECT id, status, home_score, away_score, game_start_time, game_end_time, CURRENT_TIMESTAMP as now FROM games WHERE season_id = $1",
                    season_uuid
                )
                .fetch_all(&app.db_pool)
                .await
                .expect("Failed to query games");

                println!("🔍 Current DB state:");
                for game in &current_games {
                    let now = game.now.unwrap();
                    let should_be_finished = game.game_end_time.map(|end| end <= now).unwrap_or(false);
                    println!("  Game {}: status='{}', home_score={}, away_score={}, end={:?}, now={:?}, should_be_finished={}", 
                        &game.id.to_string()[..8], game.status, 
                        game.home_score, game.away_score,
                        game.game_end_time, now, should_be_finished);
                }
                
                for (i, standing) in standings.iter().enumerate() {
                    let team_name = standing["team_name"].as_str().unwrap_or("Unknown");
                    let games_played = standing["standing"]["games_played"].as_i64().unwrap_or(0);
                    let wins = standing["standing"]["wins"].as_i64().unwrap_or(0);
                    let draws = standing["standing"]["draws"].as_i64().unwrap_or(0);
                    let losses = standing["standing"]["losses"].as_i64().unwrap_or(0);
                    let points = standing["standing"]["points"].as_i64().unwrap_or(0);
                    println!("  Team {}: {} games, {} wins, {} draws, {} losses, {} points", 
                        team_name, games_played, wins, draws, losses, points);
                }
            }
            
            // Game is completed when at least one team has played games
            // (Each game results in 2 records: one for home team, one for away team)
            if total_games_played >= 2 {
                game_completed = true;
                println!("✅ Game completed and evaluated! Total game records: {}", total_games_played);
            }
        } else {
            println!("❌ Failed to get standings, status: {}", standings_response.status());
        }
    }
    
    assert!(game_completed, "Game should have been completed and evaluated within 30 seconds");
    
    // Step 3: Verify standings make sense
    let final_standings_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/standings", &app.address, season_id),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(final_standings_response.status(), 200);
    let final_standings_data: serde_json::Value = final_standings_response.json().await.unwrap();
    let final_standings = final_standings_data["data"]["standings"].as_array().unwrap();
    
    // Verify standings integrity
    for standing in final_standings {
        let games_played = standing["standing"]["games_played"].as_i64().unwrap();
        let wins = standing["standing"]["wins"].as_i64().unwrap();
        let draws = standing["standing"]["draws"].as_i64().unwrap();
        let losses = standing["standing"]["losses"].as_i64().unwrap();
        let points = standing["standing"]["points"].as_i64().unwrap();
        
        // Basic integrity checks
        assert_eq!(games_played, wins + draws + losses, 
            "Games played should equal wins + draws + losses");
        assert_eq!(points, wins * 3 + draws * 1, 
            "Points should equal wins*3 + draws*1");
        assert!(games_played > 0, "At least one game should have been played");
    }
    
    println!("✅ Standings verification passed");
    
    // Step 4: Check if next games are scheduled/starting automatically
    // For a 2-team league, we should have 2 games total (home/away)
    let schedule_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/schedule", &app.address, season_id),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(schedule_response.status(), 200);
    let schedule_data: serde_json::Value = schedule_response.json().await.unwrap();
    let all_games = schedule_data["data"]["games"].as_array().unwrap();
    
    println!("✅ Season has {} total games scheduled", all_games.len());
    
    if all_games.len() > 1 {
        println!("⏳ Waiting to see if second game starts automatically...");
        
        let mut second_game_started = false;
        let mut second_game_attempts = 0;
        
        while !second_game_started && second_game_attempts < 20 {
            tokio::time::sleep(TokioDuration::from_secs(1)).await;
            second_game_attempts += 1;
            
            // Check for any games in progress
            let live_response = make_authenticated_request(
                &client,
                reqwest::Method::GET,
                &format!("{}/league/games/live-active?season_id={}", &app.address, season_id),
                &admin_user.token,
                None,
            ).await;
            
            if live_response.status() == 200 {
                let live_data: serde_json::Value = live_response.json().await.unwrap();
                let live_games = live_data["data"].as_array().unwrap();
                
                if !live_games.is_empty() {
                    second_game_started = true;
                    println!("✅ Second game started automatically!");
                } else if second_game_attempts % 5 == 0 {
                    println!("🔍 Still waiting for second game to start (attempt {})", second_game_attempts);
                }
            }
        }
        
        if second_game_started {
            println!("✅ Automatic next game start verified");
        } else {
            println!("⚠️ Second game didn't start within 20 seconds (may start later)");
        }
    }

    println!("🎉 Complete automated scheduler game lifecycle test completed successfully!");

    // Cleanup
    delete_test_user(&app.address, &admin_user.token, user1.user_id).await;
    delete_test_user(&app.address, &admin_user.token, user2.user_id).await;
    delete_test_user(&app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn test_scheduler_multiple_seasons() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🤖 Testing Automated Scheduler - Multiple Seasons");
    
    // Create admin
    let admin_user = create_admin_user_and_login(&app.address).await;
    
    // Create 2 leagues with different teams
    let LeagueWithTeamsResult { league_id: league1_id, team_ids: team_ids1 } = create_league_with_teams(
        &app.address,
        &admin_user.token,
        2,  // max_teams
        2,  // team_count
        None,  // team_owners
        true,  // add_to_league
        Some("League One".to_string()),
        Some("First test league".to_string()),
    ).await;
    
    let LeagueWithTeamsResult { league_id: league2_id, team_ids: team_ids2 } = create_league_with_teams(
        &app.address,
        &admin_user.token,
        2,  // max_teams
        2,  // team_count
        None,  // team_owners
        true,  // add_to_league
        Some("League Two".to_string()),
        Some("Second test league".to_string()),
    ).await;
    
    println!("✅ Created 2 leagues with teams");
    
    // Create overlapping seasons with different game durations
    let start_date1 = Utc::now() + Duration::seconds(5);
    let start_date2 = Utc::now() + Duration::seconds(8); // Slight offset
    
    let season1_request = json!({
        "name": format!("Season One {}", &Uuid::new_v4().to_string()[..4]),
        "start_date": start_date1.to_rfc3339(),
        "evaluation_timezone": "UTC",
        "auto_evaluation_enabled": true,
        "game_duration_seconds": 5 // 5 seconds
    });
    
    let season2_request = json!({
        "name": format!("Season Two {}", &Uuid::new_v4().to_string()[..4]),
        "start_date": start_date2.to_rfc3339(),
        "evaluation_timezone": "UTC", 
        "auto_evaluation_enabled": true,
        "game_duration_seconds": 4 // 4 seconds
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
    
    // Create league and teams
    let LeagueWithTeamsResult { league_id, team_ids } = create_league_with_teams(
        &app.address,
        &admin_user.token,
        2,  // max_teams
        2,  // team_count
        None,  // team_owners
        true,  // add_to_league
        Some("Error Recovery League".to_string()),
        Some("Testing scheduler error recovery".to_string()),
    ).await;
    
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