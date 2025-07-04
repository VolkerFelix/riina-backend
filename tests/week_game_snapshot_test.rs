use reqwest::Client;
use serde_json::json;
use chrono::{Utc, Duration, NaiveTime, Weekday};
use sqlx::Row;
use uuid::Uuid;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request};
use common::admin_helpers::{create_admin_user_and_login, create_league_season_with_schedule};

use crate::common::utils::get_next_date;

#[tokio::test]
async fn test_week_long_game_snapshot_system() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create admin user first
    let admin_user = create_admin_user_and_login(&test_app.address).await;

    // Create regular test users and teams
    let user1 = create_test_user_and_login(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;
    let user3 = create_test_user_and_login(&test_app.address).await;
    let user4 = create_test_user_and_login(&test_app.address).await;

    // Create league using admin user
    let league1_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", &test_app.address),
        &admin_user.token,
        Some(json!({
            "name": "Test League 1",
            "description": "A test league for snapshot testing",
            "max_teams": 4
        })),
    ).await;
    
    if !&league1_response.status().is_success() {
        panic!("League creation failed with status {}: {}", league1_response.status(), league1_response.text().await.unwrap());
    }
    
    let league1: serde_json::Value = league1_response.json().await.unwrap();
    let league1_id = league1["data"]["id"].as_str().unwrap();

    let team_name_1 = format!("Snapshot Warriors {}", &Uuid::new_v4().to_string()[..8]);
    let team_name_2 = format!("Test Titans {}", &Uuid::new_v4().to_string()[..8]);
    // Create two teams
    let team1_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", &test_app.address),
        &admin_user.token,
        Some(json!({
            "name": team_name_1,
            "color": "#FF0000",
            "owner_id": user1.user_id
        })),
    ).await;
    assert!(team1_response.status().is_success());
    let team1: serde_json::Value = team1_response.json().await.unwrap();
    let team1_id = team1["data"]["id"].as_str().unwrap();

    let team2_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", &test_app.address),
        &admin_user.token,
        Some(json!({
            "name": team_name_2, 
            "color": "#0000FF",
            "owner_id": user2.user_id
        })),
    ).await;
    assert!(team2_response.status().is_success());
    let team2: serde_json::Value = team2_response.json().await.unwrap();
    let team2_id = team2["data"]["id"].as_str().unwrap();

    // Add additional members to teams (owners are now automatically added by admin API)
    let add_user3_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams/{}/members", &test_app.address, team1_id),
        &admin_user.token,
        Some(json!({
            "user_id": user3.user_id,
            "role": "member"
        })),
    ).await;
    
    if !add_user3_response.status().is_success() {
        let error_text = add_user3_response.text().await.unwrap();
        panic!("Failed to add user3 to team1: {}", error_text);
    }

    let add_user4_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams/{}/members", &test_app.address, team2_id),
        &admin_user.token,
        Some(json!({
            "user_id": user4.user_id,
            "role": "member"
        })),
    ).await;
    
    if !add_user4_response.status().is_success() {
        let error_text = add_user4_response.text().await.unwrap();
        panic!("Failed to add user4 to team2: {}", error_text);
    }

    // Add teams to league
    let _add_team1_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/teams", &test_app.address, league1_id),
        &admin_user.token,
        Some(json!({"team_id": team1_id})),
    ).await;

    let _add_team2_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/teams", &test_app.address, league1_id),
        &admin_user.token,
        Some(json!({"team_id": team2_id})),
    ).await;

    // Set initial avatar stats for all users
    // Set Team 1 initial stats: user1(60,40), user3(50,60) = total(110,100)
    sqlx::query!(
        "UPDATE user_avatars SET stamina = $1, strength = $2 WHERE user_id = $3",
        60, 40, user1.user_id
    ).execute(&test_app.db_pool).await.unwrap();

    sqlx::query!(
        "UPDATE user_avatars SET stamina = $1, strength = $2 WHERE user_id = $3", 
        50, 60, user3.user_id
    ).execute(&test_app.db_pool).await.unwrap();

    // Set Team 2 initial stats: user2(55,45), user4(45,55) = total(100,100)
    sqlx::query!(
        "UPDATE user_avatars SET stamina = $1, strength = $2 WHERE user_id = $3",
        55, 45, user2.user_id
    ).execute(&test_app.db_pool).await.unwrap();

    sqlx::query!(
        "UPDATE user_avatars SET stamina = $1, strength = $2 WHERE user_id = $3",
        45, 55, user4.user_id
    ).execute(&test_app.db_pool).await.unwrap();

    let season_name = format!("Snapshot Test Season {}", &Uuid::new_v4().to_string()[..8]);
    let start_date = get_next_date(Weekday::Mon, NaiveTime::from_hms_opt(9, 0, 0).unwrap());

    // Create a season using admin helper with proper schedule
    let _season_id = create_league_season_with_schedule(
        &test_app.address,
        &admin_user.token,
        league1_id,
        &season_name,
        &start_date.to_rfc3339(),
        "0 0 22 * * SAT",
        Some("UTC"),
        Some(false) // Disable auto-evaluation for test
    ).await;

    // Create a week-long game manually in database
    let today = Utc::now().date_naive();
    let week_start = today;
    let week_end = today + Duration::days(7);

    let game_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO league_games 
        (id, season_id, home_team_id, away_team_id, scheduled_time, week_number, 
         is_first_leg, status, week_start_date, week_end_date)
        SELECT $1, s.id, $2, $3, $4, 1, false, 'scheduled', $5, $6
        FROM league_seasons s 
        JOIN leagues l ON s.league_id = l.id 
        WHERE l.id = $7
        "#,
        game_id,
        Uuid::parse_str(team1_id).unwrap(),
        Uuid::parse_str(team2_id).unwrap(), 
        Utc::now(),
        week_start,
        week_end,
        Uuid::parse_str(league1_id).unwrap()
    ).execute(&test_app.db_pool).await.unwrap();

    println!("🎮 Created game {} with week {} to {}", game_id, week_start, week_end);

    // Test 1: Start the game and verify snapshots are taken
    let start_response = sqlx::query!(
        "UPDATE league_games SET status = 'in_progress' WHERE id = $1 RETURNING id",
        game_id
    ).fetch_one(&test_app.db_pool).await;
    assert!(start_response.is_ok());

    // Manually take start snapshots using GameEvaluator
    use evolveme_backend::game::game_evaluator::GameEvaluator;
    
    let start_snapshot_team1 = GameEvaluator::take_team_snapshot(
        &test_app.db_pool,
        &game_id,
        &Uuid::parse_str(team1_id).unwrap(),
        "start"
    ).await.unwrap();

    let start_snapshot_team2 = GameEvaluator::take_team_snapshot(
        &test_app.db_pool,
        &game_id,
        &Uuid::parse_str(team2_id).unwrap(),
        "start" 
    ).await.unwrap();

    println!("📸 Start snapshots - Team1: {}/{}, Team2: {}/{}", 
        start_snapshot_team1.total_stamina, start_snapshot_team1.total_strength,
        start_snapshot_team2.total_stamina, start_snapshot_team2.total_strength);

    // Verify start snapshots
    assert_eq!(start_snapshot_team1.total_stamina, 110); // 60 + 50
    assert_eq!(start_snapshot_team1.total_strength, 100); // 40 + 60
    assert_eq!(start_snapshot_team2.total_stamina, 100); // 55 + 45
    assert_eq!(start_snapshot_team2.total_strength, 100); // 45 + 55

    // Test 2: Simulate workouts during the week (health data uploads)
    
    // Team 1 members do workouts - user1 gains +10 stamina, user3 gains +15 strength
    sqlx::query!(
        "UPDATE user_avatars SET stamina = stamina + 10 WHERE user_id = $1",
        user1.user_id
    ).execute(&test_app.db_pool).await.unwrap();

    sqlx::query!(
        "UPDATE user_avatars SET strength = strength + 15 WHERE user_id = $1",
        user3.user_id
    ).execute(&test_app.db_pool).await.unwrap();

    // Team 2 members do workouts - user2 gains +5 stamina, user4 gains +8 strength  
    sqlx::query!(
        "UPDATE user_avatars SET stamina = stamina + 5 WHERE user_id = $1",
        user2.user_id
    ).execute(&test_app.db_pool).await.unwrap();

    sqlx::query!(
        "UPDATE user_avatars SET strength = strength + 8 WHERE user_id = $1",
        user4.user_id
    ).execute(&test_app.db_pool).await.unwrap();

    println!("💪 Applied workout improvements during the week");

    // Test 3: End the game and take end snapshots
    let end_snapshot_team1 = GameEvaluator::take_team_snapshot(
        &test_app.db_pool,
        &game_id,
        &Uuid::parse_str(team1_id).unwrap(),
        "end"
    ).await.unwrap();

    let end_snapshot_team2 = GameEvaluator::take_team_snapshot(
        &test_app.db_pool,
        &game_id,
        &Uuid::parse_str(team2_id).unwrap(),
        "end"
    ).await.unwrap();

    println!("📸 End snapshots - Team1: {}/{}, Team2: {}/{}", 
        end_snapshot_team1.total_stamina, end_snapshot_team1.total_strength,
        end_snapshot_team2.total_stamina, end_snapshot_team2.total_strength);

    // Verify end snapshots
    assert_eq!(end_snapshot_team1.total_stamina, 120); // 110 + 10
    assert_eq!(end_snapshot_team1.total_strength, 115); // 100 + 15
    assert_eq!(end_snapshot_team2.total_stamina, 105); // 100 + 5
    assert_eq!(end_snapshot_team2.total_strength, 108); // 100 + 8

    // Test 4: Evaluate game based on snapshots
    let game_result = GameEvaluator::evaluate_game_with_snapshots(
        &test_app.db_pool,
        &game_id,
        &Uuid::parse_str(team1_id).unwrap(),
        &Uuid::parse_str(team2_id).unwrap()
    ).await.unwrap();

    println!("🏆 Game result - Team1: {} points, Team2: {} points", 
        game_result.home_score, game_result.away_score);

    // Team 1 improvements: +10 stamina + 15 strength = 25 points
    // Team 2 improvements: +5 stamina + 8 strength = 13 points
    assert_eq!(game_result.home_score, 25);
    assert_eq!(game_result.away_score, 13);
    assert_eq!(game_result.winner_team_id, Some(Uuid::parse_str(team1_id).unwrap()));

    // Test 5: Verify snapshots are stored in database
    let stored_snapshots = sqlx::query!(
        r#"
        SELECT game_id, team_id, snapshot_type, total_stamina, total_strength, member_count
        FROM game_team_snapshots 
        WHERE game_id = $1
        ORDER BY team_id, snapshot_type
        "#,
        game_id
    ).fetch_all(&test_app.db_pool).await.unwrap();

    assert_eq!(stored_snapshots.len(), 4); // 2 teams × 2 snapshots each

    // Test 6: Update game status to finished with results
    sqlx::query!(
        r#"
        UPDATE league_games 
        SET status = 'finished', home_score = $1, away_score = $2, winner_team_id = $3
        WHERE id = $4
        "#,
        game_result.home_score as i32,
        game_result.away_score as i32,
        game_result.winner_team_id,
        game_id
    ).execute(&test_app.db_pool).await.unwrap();

    let final_game = sqlx::query!(
        "SELECT status, home_score, away_score, winner_team_id FROM league_games WHERE id = $1",
        game_id
    ).fetch_one(&test_app.db_pool).await.unwrap();

    assert_eq!(final_game.status, "finished");
    assert_eq!(final_game.home_score, Some(25));
    assert_eq!(final_game.away_score, Some(13));
    assert_eq!(final_game.winner_team_id, Some(Uuid::parse_str(team1_id).unwrap()));

    println!("✅ Week-long game snapshot system test completed successfully!");
    println!("   - Start snapshots captured team stats at game beginning");
    println!("   - Teams improved stats during the week through workouts");
    println!("   - End snapshots captured final stats");
    println!("   - Winner determined by greatest improvement (Team 1: +25 vs Team 2: +13)");
}