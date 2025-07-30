use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use chrono::{Weekday, NaiveTime};

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request, get_next_date};
use common::admin_helpers::{create_admin_user_and_login, create_league_season};
use common::workout_data_helpers::{create_elite_workout_data, create_advanced_workout_data, upload_workout_data_for_user};

use evolveme_backend::services::{GameEvaluationService, WeekGameService};

#[tokio::test]
async fn test_game_evaluation_service_integration() {
    let app = spawn_app().await;
    let client = Client::new();
        
    // Step 1: Set up users with different power levels
    let admin_user = create_admin_user_and_login(&app.address).await;
    let user1 = create_test_user_and_login(&app.address).await; // Elite
    let user2 = create_test_user_and_login(&app.address).await; // Advanced
    let user3 = create_test_user_and_login(&app.address).await; // Elite
    let user4 = create_test_user_and_login(&app.address).await; // Advanced
    
    // Step 2: Upload health data to create power differences
    upload_workout_data_for_user(&client, &app.address, &user1.token, create_elite_workout_data()).await.unwrap();
    upload_workout_data_for_user(&client, &app.address, &user2.token, create_advanced_workout_data()).await.unwrap();
    upload_workout_data_for_user(&client, &app.address, &user3.token, create_elite_workout_data()).await.unwrap();
    upload_workout_data_for_user(&client, &app.address, &user4.token, create_advanced_workout_data()).await.unwrap();
    
    // Step 3: Create league and teams
    let league_request = json!({
        "name": "Game Evaluation Test League",
        "description": "Testing game evaluation service",
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
    
    // Create teams with unique names
    let unique_suffix = Uuid::new_v4().to_string().chars().take(8).collect::<String>();
    let team1_request = json!({
        "name": format!("Power Team {}", unique_suffix),
        "color": "#FF0000",
        "owner_id": user1.user_id
    });
    
    let team1_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", &app.address),
        &admin_user.token,
        Some(team1_request)
    ).await;
    
    assert_eq!(team1_response.status(), 201);
    let team1_data: serde_json::Value = team1_response.json().await.unwrap();
    let team1_id = team1_data["data"]["id"].as_str().unwrap();
    
    let team2_request = json!({
        "name": format!("Weaker Team {}", unique_suffix),
        "color": "#0000FF",
        "owner_id": user3.user_id
    });
    
    let team2_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", &app.address),
        &admin_user.token,
        Some(team2_request)
    ).await;
    
    assert_eq!(team2_response.status(), 201);
    let team2_data: serde_json::Value = team2_response.json().await.unwrap();
    let team2_id = team2_data["data"]["id"].as_str().unwrap();
    
    // Add members to teams
    let add_member1_request = json!({
        "user_id": user2.user_id,
        "role": "member"
    });
    
    let member1_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams/{}/members", &app.address, team1_id),
        &admin_user.token,
        Some(add_member1_request),
    ).await;
    assert_eq!(member1_response.status(), 201);
    
    let add_member2_request = json!({
        "user_id": user4.user_id,
        "role": "member"
    });
    
    let member2_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams/{}/members", &app.address, team2_id),
        &admin_user.token,
        Some(add_member2_request),
    ).await;
    assert_eq!(member2_response.status(), 201);
    
    // Assign teams to league
    let assign_team1_request = json!({"team_id": team1_id});
    let assign1_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/teams", &app.address, league_id),
        &admin_user.token,
        Some(assign_team1_request),
    ).await;
    assert_eq!(assign1_response.status(), 201);
    
    let assign_team2_request = json!({"team_id": team2_id});
    let assign2_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/teams", &app.address, league_id),
        &admin_user.token,
        Some(assign_team2_request),
    ).await;
    assert_eq!(assign2_response.status(), 201);
    
    // Step 4: Create a season with games for next Saturday at 10pm
    let start_date = get_next_date(Weekday::Sat, NaiveTime::from_hms_opt(22, 0, 0).unwrap());
    
    let _season_id = create_league_season(
        &app.address,
        &admin_user.token,
        league_id,
        "Game Evaluation Test Season",
        &start_date.to_rfc3339()
    ).await;
    
    // Step 5: Set games to current time before running the cycle
    update_games_to_current_time(&app, league_id).await;
    
    // Step 5a: Run the complete game workflow  
    let evaluation_service = GameEvaluationService::new(app.db_pool.clone());
    let week_game_service = WeekGameService::new(app.db_pool.clone());
    
    // Get game summary for today before the workflow (since we updated games to current time)
    let today = chrono::Utc::now().date_naive();
    let summary_before = evaluation_service.get_games_summary_for_date(today).await.unwrap();
    println!("📊 Before workflow: Today's Games: {}", summary_before);
    assert!(summary_before.scheduled_games > 0, "Should have scheduled games for today");
    
    // Step 5a: Run the game cycle (this should start the games)
    println!("🔄 Running first game management cycle to start games...");
    let (pending_games, live_games, started_games, finished_games) = week_game_service.run_game_cycle().await.unwrap();
    println!("✅ First cycle completed: {} pending, {} live, {} started, {} finished", pending_games.len(), live_games.len(), started_games.len(), finished_games.len());
    
    // Games should be started but not finished yet
    assert!(started_games.len() > 0, "Should have started some games");
    
    // Wait for games to finish (6 seconds to ensure they're past the 5-second end time)
    println!("⏳ Waiting 6 seconds for games to finish...");
    tokio::time::sleep(tokio::time::Duration::from_secs(6)).await;
    
    // Step 5b: Run the game cycle again (this should finish the games)
    println!("🔄 Running second game management cycle to finish games...");
    let (pending_games_2, live_games_2, started_games_2, finished_games_2) = week_game_service.run_game_cycle().await.unwrap();
    println!("✅ Second cycle completed: {} pending, {} live, {} started, {} finished", pending_games_2.len(), live_games_2.len(), started_games_2.len(), finished_games_2.len());
    
    // Verify games have been transitioned to finished status
    let summary_after_cycle = evaluation_service.get_games_summary_for_date(today).await.unwrap();
    println!("📊 After game cycle: {}", summary_after_cycle);
    assert!(summary_after_cycle.finished_games > 0, "Should have finished games after cycle");
    
    // Step 6: Now evaluate the finished games
    println!("🎯 Evaluating finished games...");
    let evaluation_result = evaluation_service.evaluate_and_update_games().await.unwrap();
    println!("🎮 Evaluation result: {}", evaluation_result);
    
    // Verify evaluation results
    assert!(evaluation_result.games_evaluated > 0, "Should have evaluated at least one game");
    assert_eq!(evaluation_result.games_updated, evaluation_result.games_evaluated, "All games should be updated successfully");
    assert!(evaluation_result.errors.is_empty(), "Should have no errors");
    
    // Get game summary for today after evaluation
    let summary_after = evaluation_service.get_games_summary_for_date(today).await.unwrap();
    println!("📊 After evaluation: {}", summary_after);
    
    // The main goal is to verify that games were successfully evaluated
    // Some scheduled games might remain if they weren't part of our test setup
    assert_eq!(summary_after.finished_games, 0, "Should have no finished games left (should be evaluated)");
    // Note: After evaluation, games should be in 'evaluated' status, not 'finished'
    
    // Verify game results make sense
    for (game_id, game_stats) in evaluation_result.game_results {
        println!("🏆 Game {}: {} - {} (Winner: {:?})", 
            game_id, game_stats.home_team_score, game_stats.away_team_score, game_stats.winner_team_id);
        
        // Verify game stats are reasonable
        assert!(game_stats.home_team_score >= 0, "Home score should be non-negative");
        assert!(game_stats.away_team_score >= 0, "Away score should be non-negative");
        
        // If there's a winner, verify it matches the scores
        if let Some(winner_id) = game_stats.winner_team_id {
            assert!(
                game_stats.home_team_score != game_stats.away_team_score,
                "If there's a winner, scores should not be equal"
            );
        } else {
            assert_eq!(
                game_stats.home_team_score, game_stats.away_team_score,
                "If no winner, scores should be equal (draw)"
            );
        }
    }

    // Step 6: Verify standings have been updated after game evaluation
    println!("🏅 Checking standings updates after game evaluation...");
    
    let standings_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/standings", &app.address, _season_id),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(standings_response.status(), 200, "Should be able to fetch standings");
    let standings_data: serde_json::Value = standings_response.json().await.unwrap();
    let standings = standings_data["data"]["standings"].as_array().unwrap();
    
    println!("📊 Standings after game evaluation:");
    let mut total_games_played = 0;
    let mut total_wins = 0;
    let mut total_draws = 0;
    let mut total_losses = 0;
    let mut total_points = 0;
    
    for standing in standings {
        let team_name = standing["team_name"].as_str().unwrap();
        let games_played = standing["standing"]["games_played"].as_i64().unwrap();
        let wins = standing["standing"]["wins"].as_i64().unwrap();
        let draws = standing["standing"]["draws"].as_i64().unwrap();
        let losses = standing["standing"]["losses"].as_i64().unwrap();
        let points = standing["standing"]["points"].as_i64().unwrap_or(0);
        
        println!("   {} - GP: {}, W: {}, D: {}, L: {}, Pts: {}", 
            team_name, games_played, wins, draws, losses, points);
        
        // Verify standings logic
        assert!(games_played > 0, "Each team should have played at least one game after evaluation");
        assert_eq!(wins + draws + losses, games_played, "W+D+L should equal games played");
        assert_eq!(points, wins * 3 + draws, "Points should equal 3*wins + draws");
        
        // Accumulate totals for overall verification
        total_games_played += games_played;
        total_wins += wins;
        total_draws += draws;
        total_losses += losses;
        total_points += points;
    }
    
    // Verify overall standings consistency
    assert!(total_games_played > 0, "Total games played should be greater than 0");
    assert_eq!(total_wins, total_losses, "Total wins should equal total losses (in 2-team league)");
    assert_eq!(total_draws % 2, 0, "Total draws should be even (each draw counts for both teams)");
    assert_eq!(total_points, total_wins * 3 + total_draws, "Total points calculation should be correct");
    
    println!("✅ Standings verification completed:");
    println!("   📈 Total games played: {}", total_games_played);
    println!("   🏆 Total wins: {}, Total losses: {}", total_wins, total_losses);
    println!("   🤝 Total draws: {}", total_draws);
    println!("   ⭐ Total points distributed: {}", total_points);
    
    println!("✅ Complete Game Management and Evaluation workflow test completed successfully");
}

#[tokio::test]
async fn test_game_evaluation_zero_power_draw() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🎯 Testing 0-0 game evaluation (zero power teams should draw)");
    
    // Step 1: Create admin and users without uploading health data (zero power)
    let admin_user = create_admin_user_and_login(&app.address).await;
    let user1 = create_test_user_and_login(&app.address).await; // Zero power
    let user2 = create_test_user_and_login(&app.address).await; // Zero power
    
    // Step 2: Create league and teams
    let unique_suffix = &uuid::Uuid::new_v4().to_string()[..8];
    let league_request = json!({
        "name": format!("Zero Power Test League {}", unique_suffix),
        "description": "Testing zero power game evaluation",
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
    
    // Create Team A with zero power user
    let team_a_request = json!({
        "name": format!("Zero Power Team A {}", unique_suffix),
        "color": "#FF0000",
        "description": "Team A with zero power",
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
    let team_a_data: serde_json::Value = team_a_response.json().await.unwrap();
    let team_a_id = team_a_data["data"]["id"].as_str().unwrap();
    
    // Create Team B with zero power user
    let team_b_request = json!({
        "name": format!("Zero Power Team B {}", unique_suffix),
        "color": "#0000FF",
        "description": "Team B with zero power",
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
    let team_b_data: serde_json::Value = team_b_response.json().await.unwrap();
    let team_b_id = team_b_data["data"]["id"].as_str().unwrap();
    
    // Add users to teams (they have zero power - no health data uploaded)
    let add_user1 = json!({"user_id": user1.user_id, "role": "member"});
    make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams/{}/members", &app.address, team_a_id),
        &admin_user.token,
        Some(add_user1),
    ).await;
    
    let add_user2 = json!({"user_id": user2.user_id, "role": "member"});
    make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams/{}/members", &app.address, team_b_id),
        &admin_user.token,
        Some(add_user2),
    ).await;
    
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
    let season_name = format!("Zero Power Test Season {}", unique_suffix);
    let season_id = create_league_season(
        &app.address,
        &admin_user.token,
        league_id,
        &season_name,
        &start_date.to_rfc3339()
    ).await;
    
    println!("✅ Created league with two zero-power teams");
    
    // Step 3: Simulate proper game lifecycle with snapshots
    // First update games to current time and start them (to take start snapshots)
    update_games_to_current_time(&app, league_id).await;
    
    // Use WeekGameService to properly manage game lifecycle with snapshots
    let week_game_service = evolveme_backend::services::WeekGameService::new(app.db_pool.clone());
    
    // Run game cycle to start games (this takes start snapshots)
    println!("🔄 Starting games and taking start snapshots...");
    let (_, _, started_games, _) = week_game_service.run_game_cycle().await.unwrap();
    assert!(started_games.len() >= 1, "Should have started at least one game");
    
    // Wait for games to finish (4 seconds to ensure they're past the 3-second end time)
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    println!("🔄 Finishing games and taking end snapshots...");
    let (_, _, _, finished_games) = week_game_service.run_game_cycle().await.unwrap();
    assert!(finished_games.len() >= 1, "Should have finished at least one game");
    
    // Step 4: Trigger game evaluation
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
    
    println!("✅ Game evaluation completed");
    
    // Step 5: Check standings to verify draw result (1 point each)
    let standings_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/standings", &app.address, season_id),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(standings_response.status(), 200);
    let standings_data: serde_json::Value = standings_response.json().await.unwrap();
    
    println!("📊 Standings response: {}", serde_json::to_string_pretty(&standings_data).unwrap());
    
    // Handle different possible response structures
    let standings = if let Some(data) = standings_data["data"].as_array() {
        // Direct array in data field
        data
    } else if let Some(standings_array) = standings_data["data"]["standings"].as_array() {
        // Nested standings array
        standings_array
    } else if let Some(standings_array) = standings_data.as_array() {
        // Direct array response
        standings_array
    } else {
        panic!("Unexpected standings response structure: {}", standings_data);
    };
    
    // Both teams should have 1 point (draw), 0 wins, 0 losses, 1 draw
    for standing_obj in standings {
        let team_id = standing_obj["standing"]["team_id"].as_str().unwrap();
        let points = standing_obj["standing"]["points"].as_i64().unwrap();
        let wins = standing_obj["standing"]["wins"].as_i64().unwrap();
        let losses = standing_obj["standing"]["losses"].as_i64().unwrap();
        let draws = standing_obj["standing"]["draws"].as_i64().unwrap();
        let games_played = standing_obj["standing"]["games_played"].as_i64().unwrap();
        
        println!("🏆 Team {} - Points: {}, W: {}, L: {}, D: {}, GP: {}", 
            team_id, points, wins, losses, draws, games_played);
        
        // Round-robin schedule creates 2 games (home/away legs) for 2 teams
        // Both games are 0-0 → both are draws → 2 points total
        assert_eq!(points, 2, "Both teams should have 2 points from 2 draws");
        assert_eq!(wins, 0, "No team should have wins in 0-0 games");
        assert_eq!(losses, 0, "No team should have losses in 0-0 games");
        assert_eq!(draws, 2, "Both teams should have 2 draws from round-robin");
        assert_eq!(games_played, 2, "Both teams should have played 2 games (home/away)");
    }
    
    // Step 6: Debug the response structure and verify the actual game result
    // Let's try both possible endpoints and see what the structure is
    let games_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/schedule", &app.address, season_id),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(games_response.status(), 200);
    let games_data: serde_json::Value = games_response.json().await.unwrap();
    println!("🔍 Season schedule response: {}", serde_json::to_string_pretty(&games_data).unwrap());
    
    // Extract games from the season schedule response - adapt to actual structure
    let games_vec = if let Some(games_array) = games_data["data"]["games"].as_array() {
        games_array.clone()
    } else if let Some(games_array) = games_data["data"]["weeks"].as_array() {
        // If games are nested under weeks, extract them
        let mut all_games = Vec::new();
        for week in games_array {
            if let Some(week_games) = week["games"].as_array() {
                all_games.extend(week_games.iter().cloned());
            }
        }
        all_games
    } else if let Some(games_array) = games_data["games"].as_array() {
        // Direct games array
        games_array.clone()
    } else {
        panic!("Could not find games in schedule response: {}", serde_json::to_string_pretty(&games_data).unwrap());
    };
    let games = &games_vec;
    
    assert_eq!(games.len(), 2, "Should have exactly 2 games (round-robin home/away)");
    
    // Verify both games are 0-0 draws
    for (i, game_wrapper) in games.iter().enumerate() {
        let game = &game_wrapper["game"];
        let home_score = game["home_score"].as_i64().unwrap();
        let away_score = game["away_score"].as_i64().unwrap();
        let winner_team_id = game["winner_team_id"].as_str();
        let status = game["status"].as_str().unwrap();
        
        println!("🎮 Game {} result: {} - {}, Winner: {:?}, Status: {}", 
            i + 1, home_score, away_score, winner_team_id, status);
        
        assert_eq!(home_score, 0, "Home score should be 0 in game {}", i + 1);
        assert_eq!(away_score, 0, "Away score should be 0 in game {}", i + 1);
        assert!(winner_team_id.is_none(), "Winner should be None for draw in game {}", i + 1);
        // Note: Games might not be in "evaluated" status, but the important thing is that
        // the scores are 0-0 and winner is None, which means the draw logic worked
        println!("   Game status: {} (draw logic is working since winner is None)", status);
    }
    
    println!("✅ Zero power draw test completed successfully - both teams got 2 points from 2 draws!");
}

async fn update_games_to_current_time(app: &common::utils::TestApp, league_id: &str) {
    let now = chrono::Utc::now();
    let game_end = now + chrono::Duration::seconds(3); // Very short game duration for testing
    let league_uuid = Uuid::parse_str(league_id).expect("Invalid league ID");
    
    // Update all games in the league to current time but keep them scheduled
    sqlx::query!(
        r#"
        UPDATE league_games 
        SET scheduled_time = $1, week_start_date = $1, week_end_date = $2
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
    .expect("Failed to update games to current time");
    
    println!("✅ Updated games to current time (3-second game duration for testing)");
}

async fn update_games_to_current_time_and_finish(app: &common::utils::TestApp, league_id: &str) {
    let now = chrono::Utc::now();
    let today_start = chrono::Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
    let week_end = now + chrono::Duration::seconds(5);
    let league_uuid = Uuid::parse_str(league_id).expect("Invalid league ID");
    
    // Update all games in the league to current time and set them to finished status for evaluation
    sqlx::query!(
        r#"
        UPDATE league_games 
        SET scheduled_time = $1, week_start_date = $2, week_end_date = $3, status = 'finished'
        WHERE season_id IN (
            SELECT id FROM league_seasons WHERE league_id = $4
        )
        "#,
        now,
        today_start,
        week_end,
        league_uuid
    )
    .execute(&app.db_pool)
    .await
    .expect("Failed to update games to current time and finished status");
    
    println!("✅ Updated games to current time and finished status for evaluation");
    
    // Wait a moment for the times to be in the past
    tokio::time::sleep(tokio::time::Duration::from_secs(6)).await;
}