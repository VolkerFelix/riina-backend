//! Season simulation test for 4 players and 2 teams
//! 
//! This test creates a complete season with:
//! - 4 users (2 per team)
//! - 2 teams
//! - Full round-robin schedule
//! - Simulated game results
//! - Final standings

mod common;
use common::utils::spawn_app;
use common::admin_helpers::{create_test_user_and_login_with_id, make_authenticated_request};
use serde_json::json;
use uuid::Uuid;
use rand::prelude::*;

#[tokio::test]
async fn simulate_complete_season_with_4_players_2_teams() {
    let app = spawn_app().await;
    
    println!("🏟️ Starting season simulation with 4 players and 2 teams");
    
    // Step 1: Create 4 users (2 team owners + 2 additional players)
    let (admin_token, admin_id) = create_test_user_and_login_with_id(&app.address).await;
    let (user1_token, user1_id) = create_test_user_and_login_with_id(&app.address).await;
    let (user2_token, user2_id) = create_test_user_and_login_with_id(&app.address).await;
    let (user3_token, user3_id) = create_test_user_and_login_with_id(&app.address).await;
    let (user4_token, user4_id) = create_test_user_and_login_with_id(&app.address).await;
    
    println!("✅ Created 5 users (1 admin + 4 players)");
    
    // Step 2: Create a league
    let league_request = json!({
        "name": "Simulation League",
        "description": "League for season simulation with 4 players"
    });
    
    let league_response = make_authenticated_request(
        &app.client,
        "POST",
        "/admin/leagues",
        Some(&league_request),
        &admin_token,
    ).await;
    
    assert_eq!(league_response.status(), 201);
    let league_data: serde_json::Value = league_response.json().await.unwrap();
    let league_id = league_data["data"]["id"].as_str().unwrap();
    
    println!("✅ Created league: {}", league_id);
    
    // Step 3: Create 2 teams
    let team1_request = json!({
        "name": format!("Fire Dragons {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#DC2626",
        "owner_id": user1_id
    });
    
    let team1_response = make_authenticated_request(
        &app.client,
        "POST",
        "/admin/teams",
        Some(&team1_request),
        &admin_token,
    ).await;
    
    assert_eq!(team1_response.status(), 201);
    let team1_data: serde_json::Value = team1_response.json().await.unwrap();
    let team1_id = team1_data["data"]["id"].as_str().unwrap();
    
    let team2_request = json!({
        "name": format!("Ice Warriors {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#2563EB",
        "owner_id": user3_id
    });
    
    let team2_response = make_authenticated_request(
        &app.client,
        "POST",
        "/admin/teams",
        Some(&team2_request),
        &admin_token,
    ).await;
    
    assert_eq!(team2_response.status(), 201);
    let team2_data: serde_json::Value = team2_response.json().await.unwrap();
    let team2_id = team2_data["data"]["id"].as_str().unwrap();
    
    println!("✅ Created teams: {} (Fire Dragons), {} (Ice Warriors)", team1_id, team2_id);
    
    // Step 4: Add second player to each team (team members)
    let add_member1_request = json!({
        "user_id": user2_id,
        "role": "member"
    });
    
    let member1_response = make_authenticated_request(
        &app.client,
        "POST",
        &format!("/admin/teams/{}/members", team1_id),
        Some(&add_member1_request),
        &admin_token,
    ).await;
    
    assert_eq!(member1_response.status(), 201);
    
    let add_member2_request = json!({
        "user_id": user4_id,
        "role": "member"
    });
    
    let member2_response = make_authenticated_request(
        &app.client,
        "POST",
        &format!("/admin/teams/{}/members", team2_id),
        Some(&add_member2_request),
        &admin_token,
    ).await;
    
    assert_eq!(member2_response.status(), 201);
    
    println!("✅ Added team members: User2 → Fire Dragons, User4 → Ice Warriors");
    
    // Step 5: Assign teams to league
    let assign_team1_request = json!({
        "team_id": team1_id
    });
    
    let assign1_response = make_authenticated_request(
        &app.client,
        "POST",
        &format!("/admin/leagues/{}/teams", league_id),
        Some(&assign_team1_request),
        &admin_token,
    ).await;
    
    assert_eq!(assign1_response.status(), 200);
    
    let assign_team2_request = json!({
        "team_id": team2_id
    });
    
    let assign2_response = make_authenticated_request(
        &app.client,
        "POST",
        &format!("/admin/leagues/{}/teams", league_id),
        Some(&assign_team2_request),
        &admin_token,
    ).await;
    
    assert_eq!(assign2_response.status(), 200);
    
    println!("✅ Assigned both teams to league");
    
    // Step 6: Create a season
    let season_request = json!({
        "name": "Simulation Season 2025",
        "start_date": "2025-06-01T00:00:00Z",
        "end_date": "2025-08-31T23:59:59Z"
    });
    
    let season_response = make_authenticated_request(
        &app.client,
        "POST",
        &format!("/admin/leagues/{}/seasons", league_id),
        Some(&season_request),
        &admin_token,
    ).await;
    
    assert_eq!(season_response.status(), 201);
    let season_data: serde_json::Value = season_response.json().await.unwrap();
    let season_id = season_data["data"]["id"].as_str().unwrap();
    
    println!("✅ Created season: {}", season_id);
    
    // Step 7: Generate schedule
    let schedule_response = make_authenticated_request(
        &app.client,
        "POST",
        &format!("/admin/leagues/{}/schedule", league_id),
        None::<&serde_json::Value>,
        &admin_token,
    ).await;
    
    assert_eq!(schedule_response.status(), 200);
    let schedule_data: serde_json::Value = schedule_response.json().await.unwrap();
    let games = schedule_data["data"]["games"].as_array().unwrap();
    
    println!("✅ Generated schedule with {} games", games.len());
    
    // Step 8: Simulate game results
    println!("🎮 Simulating game results...");
    
    let mut rng = thread_rng();
    let mut simulated_games = 0;
    
    for game in games {
        let game_id = game["id"].as_str().unwrap();
        let home_team_id = game["home_team_id"].as_str().unwrap();
        let away_team_id = game["away_team_id"].as_str().unwrap();
        
        // Simulate realistic scores (0-5 goals each team)
        let home_score = rng.gen_range(0..=5);
        let away_score = rng.gen_range(0..=5);
        
        // Determine winner
        let winner_team_id = if home_score > away_score {
            Some(home_team_id)
        } else if away_score > home_score {
            Some(away_team_id)
        } else {
            None // Draw
        };
        
        let result_request = json!({
            "home_score": home_score,
            "away_score": away_score,
            "winner_team_id": winner_team_id,
            "status": "finished"
        });
        
        let result_response = make_authenticated_request(
            &app.client,
            "PUT",
            &format!("/admin/games/{}/result", game_id),
            Some(&result_request),
            &admin_token,
        ).await;
        
        assert_eq!(result_response.status(), 200);
        simulated_games += 1;
        
        let home_team_name = if home_team_id == team1_id { "Fire Dragons" } else { "Ice Warriors" };
        let away_team_name = if away_team_id == team1_id { "Fire Dragons" } else { "Ice Warriors" };
        
        println!("   {} {} - {} {} (Game {})", 
                 home_team_name, home_score, 
                 away_score, away_team_name, 
                 simulated_games);
    }
    
    println!("✅ Simulated {} games", simulated_games);
    
    // Step 9: Get final standings
    let standings_response = make_authenticated_request(
        &app.client,
        "GET",
        &format!("/league/seasons/{}/standings", season_id),
        None::<&serde_json::Value>,
        &admin_token,
    ).await;
    
    assert_eq!(standings_response.status(), 200);
    let standings_data: serde_json::Value = standings_response.json().await.unwrap();
    let standings = standings_data["data"]["standings"].as_array().unwrap();
    
    println!("\n🏆 FINAL STANDINGS:");
    println!("Pos | Team           | GP | W | D | L | Pts");
    println!("----|----------------|----|----|----|----|----");
    
    for standing in standings {
        let position = standing["standing"]["position"].as_i64().unwrap();
        let team_name = standing["team_name"].as_str().unwrap();
        let games_played = standing["standing"]["games_played"].as_i64().unwrap();
        let wins = standing["standing"]["wins"].as_i64().unwrap();
        let draws = standing["standing"]["draws"].as_i64().unwrap();
        let losses = standing["standing"]["losses"].as_i64().unwrap();
        let points = standing["standing"]["points"].as_i64().unwrap_or(0);
        
        println!("{:3} | {:14} | {:2} | {:2} | {:2} | {:2} | {:3}",
                 position, team_name, games_played, wins, draws, losses, points);
    }
    
    // Step 10: Verify season integrity
    println!("\n🔍 Season verification:");
    
    // Check that each team played the expected number of games
    for standing in standings {
        let games_played = standing["standing"]["games_played"].as_i64().unwrap();
        // With 2 teams, each team should play each other multiple times in a round-robin
        assert!(games_played > 0, "Each team should have played at least one game");
        println!("   ✓ Team played {} games", games_played);
    }
    
    // Check points calculation (3 for win, 1 for draw, 0 for loss)
    for standing in standings {
        let wins = standing["standing"]["wins"].as_i64().unwrap();
        let draws = standing["standing"]["draws"].as_i64().unwrap();
        let expected_points = wins * 3 + draws * 1;
        let actual_points = standing["standing"]["points"].as_i64().unwrap_or(0);
        assert_eq!(expected_points, actual_points, "Points calculation should be correct");
        println!("   ✓ Points correctly calculated: {} wins × 3 + {} draws × 1 = {} points", 
                 wins, draws, actual_points);
    }
    
    println!("\n🎉 Season simulation completed successfully!");
    println!("   • 4 players created and assigned to teams");
    println!("   • 2 teams competing in league");
    println!("   • {} games simulated with realistic scores", simulated_games);
    println!("   • Final standings calculated correctly");
    println!("   • All data integrity checks passed");
}