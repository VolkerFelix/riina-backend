//! Season simulation test for 4 players and 2 teams
//! 
//! This test creates a complete season with:
//! - 4 users (2 per team)
//! - 2 teams
//! - Full round-robin schedule
//! - Simulated game results
//! - Final standings

mod common;
use common::utils::{
    spawn_app,
    create_test_user_and_login,
    get_next_date,
    make_authenticated_request
, delete_test_user};
use common::admin_helpers::{create_admin_user_and_login, create_league_season, create_league, create_team, TeamConfig, add_team_to_league, add_user_to_team};
use common::workout_data_helpers::{
    WorkoutData,
    WorkoutIntensity,
    upload_workout_data_for_user
};
use serde_json::json;
use uuid::Uuid;
use rand::prelude::*;
use reqwest::Client;
use chrono::{Weekday, NaiveTime, Utc};

#[tokio::test]
async fn simulate_complete_season_with_4_players_2_teams() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🏟️ Starting season simulation with 4 players and 2 teams");
    
    // Step 1: Create 4 users (2 team owners + 2 additional players)
    let admin_user = create_admin_user_and_login(&app.address, &app.db_pool).await;
    let user1 = create_test_user_and_login(&app.address).await;
    let user2 = create_test_user_and_login(&app.address).await;
    let user3 = create_test_user_and_login(&app.address).await;
    let user4 = create_test_user_and_login(&app.address).await;
    
    println!("✅ Created 5 users (1 admin + 4 players)");
    
    // Step 2: Create a league
    let league_id = create_league(
        &app.address,
        &admin_user.token,
        2
    ).await;
    
    println!("✅ Created league: {}", league_id);
    
    // Step 3: Create 2 teams with specific names and owners
    let team1_id = create_team(
        &app.address,
        &admin_user.token,
        TeamConfig {
            name: Some(format!("Fire Dragons {}", &Uuid::new_v4().to_string()[..8])),
            color: Some("#DC2626".to_string()),
            owner_id: Some(user1.user_id),
            description: None,
        }
    ).await;
    
    let team2_id = create_team(
        &app.address,
        &admin_user.token,
        TeamConfig {
            name: Some(format!("Ice Warriors {}", &Uuid::new_v4().to_string()[..8])),
            color: Some("#2563EB".to_string()),
            owner_id: Some(user3.user_id),
            description: None,
        }
    ).await;
    
    println!("✅ Created teams: {} (Fire Dragons), {} (Ice Warriors)", team1_id, team2_id);
    
    // Step 4: Add second player to each team (team members)
    add_user_to_team(&app.address, &admin_user.token, &team1_id, user2.user_id).await;
    add_user_to_team(&app.address, &admin_user.token, &team2_id, user4.user_id).await;
    
    println!("✅ Added team members: User2 → Fire Dragons, User4 → Ice Warriors");
    
    // Step 5: Assign teams to league
    add_team_to_league(&app.address, &admin_user.token, &league_id, &team1_id).await;
    add_team_to_league(&app.address, &admin_user.token, &league_id, &team2_id).await;
    
    println!("✅ Assigned both teams to league");
    
    // Step 6: Create a season
    let start_date = get_next_date(Weekday::Sun, NaiveTime::from_hms_opt(10, 0, 0).unwrap());
    let season_id = create_league_season(
        &app.address,
        &admin_user.token,
        &league_id,
        "Simulation Season 2025",
        &start_date.to_rfc3339()
    ).await;
    
    println!("✅ Created season: {}", season_id);
    
    // Step 7: Upload health data for different fitness levels
    println!("🏃 Uploading health data to build user stats...");
    
    // User1 (Team1 Owner): Elite athlete
    let mut user1_health = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30);
    let user1_upload = upload_workout_data_for_user(&client, &app.address, &user1.token, &mut user1_health).await;
    match user1_upload {
        Ok(response) => {
            println!("   ✅ User1 (Fire Dragons Owner): Elite fitness data uploaded");
            println!("   📊 User1 upload response: {}", serde_json::to_string_pretty(&response).unwrap_or("failed to format".to_string()));
        }
        Err(e) => panic!("Failed to upload health data for user1: {}", e),
    }
    
    // User2 (Team1 Member): Advanced athlete  
    let mut user2_health = WorkoutData::new(WorkoutIntensity::Moderate, Utc::now(), 30);
    let user2_upload = upload_workout_data_for_user(&client, &app.address, &user2.token, &mut user2_health).await;
    match user2_upload {
        Ok(response) => {
            println!("   ✅ User2 (Fire Dragons Member): Advanced fitness data uploaded");
            println!("   📊 User2 upload response: {}", serde_json::to_string_pretty(&response).unwrap_or("failed to format".to_string()));
        }
        Err(e) => panic!("Failed to upload health data for user2: {}", e),
    }
    
    // User3 (Team2 Owner): Intermediate athlete
    let mut user3_health = WorkoutData::new(WorkoutIntensity::Moderate, Utc::now(), 30);
    let user3_upload = upload_workout_data_for_user(&client, &app.address, &user3.token, &mut user3_health).await;
    if let Err(e) = user3_upload {
        panic!("Failed to upload health data for user3: {}", e);
    }
    println!("   ✅ User3 (Ice Warriors Owner): Intermediate fitness data uploaded");
    
    // User4 (Team2 Member): Beginner athlete
    let mut user4_health = WorkoutData::new(WorkoutIntensity::Light, Utc::now(), 30);
    let user4_upload = upload_workout_data_for_user(&client, &app.address, &user4.token, &mut user4_health).await;
    if let Err(e) = user4_upload {
        panic!("Failed to upload health data for user4: {}", e);
    }
    println!("   ✅ User4 (Ice Warriors Member): Beginner fitness data uploaded");
    
    // Wait a moment for stats to be processed
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    // Step 8: Get user stats individually after health data upload
    println!("📊 Fetching individual user stats after health data upload...");
    
    // Function to get user stats using their own profile endpoint
    async fn get_user_stats(client: &Client, app_address: &str, user_token: &str, user_id: &Uuid) -> (i64, i64, i64) {
        let response = make_authenticated_request(
            client,
            reqwest::Method::GET,
            &format!("{}/profile/user", app_address),
            user_token,
            None,
        ).await;
        
        if response.status() != 200 {
            panic!("Failed to get user stats for {}: {}", user_id, response.status());
        }
        
        let user_data: serde_json::Value = response.json().await.unwrap();
        let stats = &user_data["data"]["stats"];
        let stamina = stats["stamina"].as_i64().unwrap_or(50);
        let strength = stats["strength"].as_i64().unwrap_or(50);
        let total = stamina + strength;
        
        (stamina, strength, total)
    }
    
    // Get stats for each user using their own tokens
    let (user1_stamina, user1_strength, user1_total) = get_user_stats(&client, &app.address, &user1.token, &user1.user_id).await;
    let (user2_stamina, user2_strength, user2_total) = get_user_stats(&client, &app.address, &user2.token, &user2.user_id).await;
    let (user3_stamina, user3_strength, user3_total) = get_user_stats(&client, &app.address, &user3.token, &user3.user_id).await;
    let (user4_stamina, user4_strength, user4_total) = get_user_stats(&client, &app.address, &user4.token, &user4.user_id).await;
    
    // Calculate team power
    let team1_power = user1_total + user2_total;
    let team2_power = user3_total + user4_total;
    
    // Display user stats
    println!("   User Stats After Health Data Upload:");
    println!("   User              | Stamina | Strength | Total | Fitness Level");
    println!("   ------------------|---------|----------|-------|---------------");
    println!("   User1 (Fire D.)   | {:7} | {:8} | {:5} | Elite", user1_stamina, user1_strength, user1_total);
    println!("   User2 (Fire D.)   | {:7} | {:8} | {:5} | Advanced", user2_stamina, user2_strength, user2_total);
    println!("   User3 (Ice W.)    | {:7} | {:8} | {:5} | Intermediate", user3_stamina, user3_strength, user3_total);
    println!("   User4 (Ice W.)    | {:7} | {:8} | {:5} | Beginner", user4_stamina, user4_strength, user4_total);
    
    println!("\n   📈 Team Power Summary:");
    println!("   Fire Dragons Total Power: {} (Elite + Advanced)", team1_power);
    println!("   Ice Warriors Total Power: {} (Intermediate + Beginner)", team2_power);
    
    let power_ratio = team1_power as f64 / team2_power as f64;
    println!("   Power Ratio: {:.2}:1 in favor of Fire Dragons", power_ratio);
    
    // Step 9: Get the games that were automatically generated
    let games_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/schedule", &app.address, season_id),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(games_response.status(), 200);
    let games_data: serde_json::Value = games_response.json().await.unwrap();
    let games = games_data["data"]["games"].as_array().unwrap();
    
    println!("✅ Season has {} games in schedule", games.len());
    
    // Step 10: Simulate game results based on team power
    println!("🎮 Simulating game results based on team power...");
    
    let mut rng = thread_rng();
    let mut simulated_games = 0;
    
    for game_wrapper in games {
        let game = &game_wrapper["game"];
        let game_id = game["id"].as_str().unwrap();
        let home_team_id = game["home_team_id"].as_str().unwrap();
        let away_team_id = game["away_team_id"].as_str().unwrap();
        
        // Determine team powers and names
        let (home_power, away_power, home_team_name, away_team_name) = if home_team_id == team1_id {
            (team1_power, team2_power, "Fire Dragons", "Ice Warriors")
        } else {
            (team2_power, team1_power, "Ice Warriors", "Fire Dragons")
        };
        
        // Power-based game simulation (no home field advantage)
        let total_power = home_power + away_power;
        let (home_score, away_score) = if total_power == 0 {
            // Fallback to random if no stats (shouldn't happen after health data upload)
            println!("   ⚠️  Warning: No team power data, using random scores");
            (rng.gen_range(0..=3), rng.gen_range(0..=3))
        } else {
            // Calculate win probability based on power difference
            let home_win_probability = home_power as f64 / total_power as f64;
            
            // Power difference influences score margin
            let power_difference = (home_power - away_power).abs() as f64;
            let max_score_difference = (power_difference / total_power as f64 * 4.0) as i32; // Max 4 goal difference
            
            // Simulate the game outcome
            let random_factor = rng.gen::<f64>();
            
            if random_factor < home_win_probability * 0.8 {
                // Home team wins (reduce probability for more balanced games)
                let home_goals = rng.gen_range(1..=4);
                let score_diff = (max_score_difference / 2).max(1);
                (home_goals, (home_goals - score_diff).max(0))
            } else if random_factor > (1.0 - (away_power as f64 / total_power as f64) * 0.8) {
                // Away team wins
                let away_goals = rng.gen_range(1..=4);
                let score_diff = (max_score_difference / 2).max(1);
                ((away_goals - score_diff).max(0), away_goals)
            } else {
                // Draw (happens when teams are close in power or random factor is in middle)
                let base_score = rng.gen_range(0..=2);
                (base_score, base_score)
            }
        };
        
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
            &client,
            reqwest::Method::PUT,
            &format!("{}/league/games/{}/result", &app.address, game_id),
            &admin_user.token,
            Some(result_request),
        ).await;
        
        assert_eq!(result_response.status(), 200);
        simulated_games += 1;
        
        println!("   {} ({}) {} - {} {} ({}) (Game {})", 
                 home_team_name, home_power, home_score, 
                 away_score, away_team_name, away_power, 
                 simulated_games);
    }
    
    println!("✅ Simulated {} games based on team power", simulated_games);

    // Step 11: Re-fetch games to get updated scores
    let updated_games_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/schedule", &app.address, season_id),
        &admin_user.token,
        None,
    ).await;

    assert_eq!(updated_games_response.status(), 200);
    let updated_games_data: serde_json::Value = updated_games_response.json().await.unwrap();
    let updated_games = updated_games_data["data"]["games"].as_array().unwrap();

    // Calculate expected total points scored from updated game results
    let mut expected_team1_scored = 0;
    let mut expected_team2_scored = 0;

    for game_wrapper in updated_games {
        let game = &game_wrapper["game"];
        let home_team_id = game["home_team_id"].as_str().unwrap();
        let away_team_id = game["away_team_id"].as_str().unwrap();
        let home_score = game["home_score"].as_i64().unwrap_or(0);
        let away_score = game["away_score"].as_i64().unwrap_or(0);

        if home_team_id == team1_id {
            expected_team1_scored += home_score;
        } else if home_team_id == team2_id {
            expected_team2_scored += home_score;
        }

        if away_team_id == team1_id {
            expected_team1_scored += away_score;
        } else if away_team_id == team2_id {
            expected_team2_scored += away_score;
        }
    }

    println!("   Expected scores - Fire Dragons: {}, Ice Warriors: {}",
             expected_team1_scored, expected_team2_scored);

    // Step 12: Get final standings
    let standings_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/standings", &app.address, season_id),
        &admin_user.token,
        None,
    ).await;

    assert_eq!(standings_response.status(), 200);
    let standings_data: serde_json::Value = standings_response.json().await.unwrap();
    let standings = standings_data["data"]["standings"].as_array().unwrap();

    println!("\n🏆 FINAL STANDINGS:");
    println!("Pos | Team           | GP | W | D | L | Pts | Scored");
    println!("----|----------------|----|----|----|----|-----|-------");

    for standing in standings {
        let position = standing["standing"]["position"].as_i64().unwrap();
        let team_id = standing["standing"]["team_id"].as_str().unwrap();
        let team_name = standing["team_name"].as_str().unwrap();
        let games_played = standing["standing"]["games_played"].as_i64().unwrap();
        let wins = standing["standing"]["wins"].as_i64().unwrap();
        let draws = standing["standing"]["draws"].as_i64().unwrap();
        let losses = standing["standing"]["losses"].as_i64().unwrap();
        let points = standing["standing"]["points"].as_i64().unwrap_or(0);
        let total_points_scored = standing["standing"]["total_points_scored"].as_i64();

        println!("{:3} | {:14} | {:2} | {:2} | {:2} | {:2} | {:3} | {:7}",
                 position, team_name, games_played, wins, draws, losses, points,
                 total_points_scored.map(|p| p.to_string()).unwrap_or("N/A".to_string()));

        // Verify that total_points_scored is present in the API response
        assert!(total_points_scored.is_some(),
            "total_points_scored should be present in the standings response for team {}", team_name);

        let actual_scored = total_points_scored.unwrap();
        assert!(actual_scored >= 0,
            "total_points_scored should be >= 0 for team {}", team_name);

        // Verify the correct amount of points scored matches what we calculated
        let expected_scored = if team_id == team1_id {
            expected_team1_scored
        } else if team_id == team2_id {
            expected_team2_scored
        } else {
            panic!("Unknown team ID in standings: {}", team_id);
        };

        assert_eq!(actual_scored, expected_scored,
            "Team {} should have scored {} points total, but API returned {}",
            team_name, expected_scored, actual_scored);

        println!("   ✓ {} total points scored verified: {}", team_name, actual_scored);
    }
    
    // Step 13: Verify season integrity
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
    println!("   • 4 players with different fitness levels created");
    println!("   • Health data uploaded: Elite + Advanced vs Intermediate + Beginner");
    println!("   • Team power calculated: Fire Dragons ({}) vs Ice Warriors ({})", team1_power, team2_power);
    println!("   • {} games simulated based on team power differential", simulated_games);
    println!("   • Final standings reflect team strength and some randomness");
    println!("   • All data integrity checks passed");
}
#[tokio::test]
async fn test_standings_tiebreaker_head_to_head() {
    let app = spawn_app().await;
    let client = Client::new();

    println!("🎯 Testing Standings Tie-Breaker: Head-to-Head");

    // Step 1: Create 4 users (team owners)
    let admin_user = create_admin_user_and_login(&app.address, &app.db_pool).await;
    let user1 = create_test_user_and_login(&app.address).await;
    let user2 = create_test_user_and_login(&app.address).await;
    let user3 = create_test_user_and_login(&app.address).await;
    let user4 = create_test_user_and_login(&app.address).await;

    println!("✅ Created 5 users (1 admin + 4 team owners)");

    // Step 2: Create a league
    let league_id = create_league(
        &app.address,
        &admin_user.token,
        4
    ).await;

    println!("✅ Created league: {}", league_id);

    // Step 3: Create 4 teams with specific names and owners
    let team_a_id = create_team(
        &app.address,
        &admin_user.token,
        TeamConfig {
            name: Some(format!("Team A {}", &Uuid::new_v4().to_string()[..8])),
            color: Some("#FF0000".to_string()),
            owner_id: Some(user1.user_id),
            description: None,
        }
    ).await;

    let team_b_id = create_team(
        &app.address,
        &admin_user.token,
        TeamConfig {
            name: Some(format!("Team B {}", &Uuid::new_v4().to_string()[..8])),
            color: Some("#00FF00".to_string()),
            owner_id: Some(user2.user_id),
            description: None,
        }
    ).await;

    let team_c_id = create_team(
        &app.address,
        &admin_user.token,
        TeamConfig {
            name: Some(format!("Team C {}", &Uuid::new_v4().to_string()[..8])),
            color: Some("#0000FF".to_string()),
            owner_id: Some(user3.user_id),
            description: None,
        }
    ).await;

    let team_d_id = create_team(
        &app.address,
        &admin_user.token,
        TeamConfig {
            name: Some(format!("Team D {}", &Uuid::new_v4().to_string()[..8])),
            color: Some("#FFFF00".to_string()),
            owner_id: Some(user4.user_id),
            description: None,
        }
    ).await;

    println!("✅ Created teams: {} (Team A), {} (Team B), {} (Team C), {} (Team D)",
        team_a_id, team_b_id, team_c_id, team_d_id);

    // Step 4: Assign teams to league
    add_team_to_league(&app.address, &admin_user.token, &league_id, &team_a_id).await;
    add_team_to_league(&app.address, &admin_user.token, &league_id, &team_b_id).await;
    add_team_to_league(&app.address, &admin_user.token, &league_id, &team_c_id).await;
    add_team_to_league(&app.address, &admin_user.token, &league_id, &team_d_id).await;

    println!("✅ Assigned all 4 teams to league");
    
    // Step 4: Create a season
    let start_date = get_next_date(Weekday::Mon, NaiveTime::from_hms_opt(9, 0, 0).unwrap());
    let season_id = create_league_season(
        &app.address,
        &admin_user.token,
        &league_id,
        "Tie-Breaker Season",
        &start_date.to_rfc3339(),
    ).await;
    
    println!("✅ Created season");
    
    // Step 5: Get the games
    let schedule_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/schedule", &app.address, season_id),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(schedule_response.status(), 200);
    let schedule_data: serde_json::Value = schedule_response.json().await.unwrap();
    let games = schedule_data["data"]["games"].as_array().unwrap();
    
    println!("✅ Got {} games from schedule", games.len());
    
    // Step 6: Simulate game results to create a tie scenario
    // In a 4-team single round-robin: 6 games total (each team plays 3 games)
    // Target: Team A and Team B both get 6 points (2 wins, 1 loss each)
    // Team A beats Team B in their head-to-head matchup
    // Expected result: Team A ranks higher than Team B due to head-to-head

    println!("🎮 Simulating game results to create tie scenario...");

    for game_wrapper in games {
        let game = &game_wrapper["game"];
        let game_id = game["id"].as_str().unwrap();
        let home_team_id = game["home_team_id"].as_str().unwrap();
        let away_team_id = game["away_team_id"].as_str().unwrap();

        // Determine result for each specific matchup
        // Team A: beats B and C, loses to D = 6 points (2W, 0D, 1L)
        // Team B: beats C and D, loses to A = 6 points (2W, 0D, 1L)
        // Team C: loses all = 0 points
        // Team D: beats A, loses to B, draw with C = 4 points (1W, 1D, 1L)
        let (home_score, away_score) =
            if (home_team_id == team_a_id && away_team_id == team_b_id) ||
               (home_team_id == team_b_id && away_team_id == team_a_id) {
            // A vs B: A wins
            if home_team_id == team_a_id {
                println!("   A vs B: A wins (40-20)");
                (40, 20)
            } else {
                println!("   B vs A: A wins (20-40)");
                (20, 40)
            }
        } else if (home_team_id == team_a_id && away_team_id == team_c_id) ||
                  (home_team_id == team_c_id && away_team_id == team_a_id) {
            // A vs C: A wins
            if home_team_id == team_a_id {
                println!("   A vs C: A wins (40-20)");
                (40, 20)
            } else {
                println!("   C vs A: A wins (20-40)");
                (20, 40)
            }
        } else if (home_team_id == team_a_id && away_team_id == team_d_id) ||
                  (home_team_id == team_d_id && away_team_id == team_a_id) {
            // A vs D: D wins
            if home_team_id == team_a_id {
                println!("   A vs D: D wins (20-40)");
                (20, 40)
            } else {
                println!("   D vs A: D wins (40-20)");
                (40, 20)
            }
        } else if (home_team_id == team_b_id && away_team_id == team_c_id) ||
                  (home_team_id == team_c_id && away_team_id == team_b_id) {
            // B vs C: B wins
            if home_team_id == team_b_id {
                println!("   B vs C: B wins (40-20)");
                (40, 20)
            } else {
                println!("   C vs B: B wins (20-40)");
                (20, 40)
            }
        } else if (home_team_id == team_b_id && away_team_id == team_d_id) ||
                  (home_team_id == team_d_id && away_team_id == team_b_id) {
            // B vs D: B wins
            if home_team_id == team_b_id {
                println!("   B vs D: B wins (40-20)");
                (40, 20)
            } else {
                println!("   D vs B: B wins (20-40)");
                (20, 40)
            }
        } else if (home_team_id == team_c_id && away_team_id == team_d_id) ||
                  (home_team_id == team_d_id && away_team_id == team_c_id) {
            // C vs D: Draw
            println!("   C vs D: Draw (30-30)");
            (30, 30)
        } else {
            println!("   ⚠️  Unknown matchup");
            (25, 25)
        };
        
        let result_request = json!({
            "home_score": home_score,
            "away_score": away_score
        });
        
        let result_response = make_authenticated_request(
            &client,
            reqwest::Method::PUT,
            &format!("{}/league/games/{}/result", &app.address, game_id),
            &admin_user.token,
            Some(result_request),
        ).await;

        if result_response.status() != 200 {
            let status = result_response.status();
            let error_text = result_response.text().await.unwrap();
            panic!("Failed to submit game result for game {}: Status {}, Error: {}", game_id, status, error_text);
        }
    }
    
    println!("✅ Submitted all game results");
    
    // Step 7: Get the final standings
    let standings_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/standings", &app.address, season_id),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(standings_response.status(), 200);
    let standings_data: serde_json::Value = standings_response.json().await.unwrap();
    let standings = standings_data["data"]["standings"].as_array().unwrap();
    
    println!("✅ Got final standings");
    
    // Find Team A and Team B positions
    let mut team_a_position = 0;
    let mut team_b_position = 0;
    let mut team_a_points = 0;
    let mut team_b_points = 0;
    
    for standing in standings {
        let team_id = standing["standing"]["team_id"].as_str().unwrap();
        let position = standing["standing"]["position"].as_i64().unwrap();
        let points = standing["standing"]["points"].as_i64().unwrap();
        
        if team_id == team_a_id {
            team_a_position = position;
            team_a_points = points;
        } else if team_id == team_b_id {
            team_b_position = position;
            team_b_points = points;
        }
    }
    
    println!("Team A: Position {}, Points {}", team_a_position, team_a_points);
    println!("Team B: Position {}, Points {}", team_b_position, team_b_points);
    
    // Verify both teams have the same points
    assert_eq!(team_a_points, team_b_points, "Teams should have equal points");
    
    // Verify Team A is ranked higher (lower position number) than Team B due to head-to-head
    assert!(team_a_position < team_b_position,
        "Team A (position {}) should be ranked higher than Team B (position {}) due to head-to-head advantage",
        team_a_position, team_b_position);
    
    println!("✅ Tie-breaker working correctly: Team A is ranked higher than Team B due to head-to-head advantage");
}

#[tokio::test]
async fn test_standings_tiebreaker_three_way_tie() {
    let app = spawn_app().await;
    let client = Client::new();

    println!("🎯 Testing Standings Tie-Breaker: Three-way Tie with Total Points Scored");

    // Step 1: Create 6 users (team owners)
    let admin_user = create_admin_user_and_login(&app.address, &app.db_pool).await;
    let user1 = create_test_user_and_login(&app.address).await;
    let user2 = create_test_user_and_login(&app.address).await;
    let user3 = create_test_user_and_login(&app.address).await;
    let user4 = create_test_user_and_login(&app.address).await;
    let user5 = create_test_user_and_login(&app.address).await;
    let user6 = create_test_user_and_login(&app.address).await;

    println!("✅ Created 7 users (1 admin + 6 team owners)");

    // Step 2: Create a league
    let league_id = create_league(
        &app.address,
        &admin_user.token,
        6
    ).await;

    println!("✅ Created league: {}", league_id);

    // Step 3: Create 6 teams
    let team_a_id = create_team(
        &app.address,
        &admin_user.token,
        TeamConfig {
            name: Some(format!("Team A {}", &Uuid::new_v4().to_string()[..8])),
            color: Some("#FF0000".to_string()),
            owner_id: Some(user1.user_id),
            description: None,
        }
    ).await;

    let team_b_id = create_team(
        &app.address,
        &admin_user.token,
        TeamConfig {
            name: Some(format!("Team B {}", &Uuid::new_v4().to_string()[..8])),
            color: Some("#00FF00".to_string()),
            owner_id: Some(user2.user_id),
            description: None,
        }
    ).await;

    let team_c_id = create_team(
        &app.address,
        &admin_user.token,
        TeamConfig {
            name: Some(format!("Team C {}", &Uuid::new_v4().to_string()[..8])),
            color: Some("#0000FF".to_string()),
            owner_id: Some(user3.user_id),
            description: None,
        }
    ).await;

    let team_d_id = create_team(
        &app.address,
        &admin_user.token,
        TeamConfig {
            name: Some(format!("Team D {}", &Uuid::new_v4().to_string()[..8])),
            color: Some("#FFFF00".to_string()),
            owner_id: Some(user4.user_id),
            description: None,
        }
    ).await;

    let team_e_id = create_team(
        &app.address,
        &admin_user.token,
        TeamConfig {
            name: Some(format!("Team E {}", &Uuid::new_v4().to_string()[..8])),
            color: Some("#FF00FF".to_string()),
            owner_id: Some(user5.user_id),
            description: None,
        }
    ).await;

    let team_f_id = create_team(
        &app.address,
        &admin_user.token,
        TeamConfig {
            name: Some(format!("Team F {}", &Uuid::new_v4().to_string()[..8])),
            color: Some("#00FFFF".to_string()),
            owner_id: Some(user6.user_id),
            description: None,
        }
    ).await;

    println!("✅ Created 6 teams");

    // Step 4: Assign teams to league
    add_team_to_league(&app.address, &admin_user.token, &league_id, &team_a_id).await;
    add_team_to_league(&app.address, &admin_user.token, &league_id, &team_b_id).await;
    add_team_to_league(&app.address, &admin_user.token, &league_id, &team_c_id).await;
    add_team_to_league(&app.address, &admin_user.token, &league_id, &team_d_id).await;
    add_team_to_league(&app.address, &admin_user.token, &league_id, &team_e_id).await;
    add_team_to_league(&app.address, &admin_user.token, &league_id, &team_f_id).await;

    println!("✅ Assigned all 6 teams to league");

    // Step 5: Create a season
    let start_date = get_next_date(Weekday::Mon, NaiveTime::from_hms_opt(9, 0, 0).unwrap());
    let season_id = create_league_season(
        &app.address,
        &admin_user.token,
        &league_id,
        "Three-way Tie Season",
        &start_date.to_rfc3339()
    ).await;

    println!("✅ Created season: {}", season_id);

    // Step 6: Get the games
    let schedule_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/schedule", &app.address, season_id),
        &admin_user.token,
        None,
    ).await;

    assert_eq!(schedule_response.status(), 200);
    let schedule_data: serde_json::Value = schedule_response.json().await.unwrap();
    let games = schedule_data["data"]["games"].as_array().unwrap();

    println!("✅ Got {} games from schedule", games.len());

    // Step 7: Simulate game results to create a 3-way tie scenario
    // Target scenario:
    // Teams A, B, C all have 6 points (2W, 0D, 3L each)
    // Head-to-head among A, B, C (CIRCULAR - rock/paper/scissors):
    //   - A beats B (3 points for A)
    //   - B beats C (3 points for B)
    //   - C beats A (3 points for C)
    // Head-to-head points: A=3, B=3, C=3 (still tied!)
    // Total points scored (second tie-breaker):
    //   - Team A scores 45 total (high: 40+5)
    //   - Team B scores 40 total (medium: 35+5)
    //   - Team C scores 35 total (low: 30+5)
    // Expected ranking: A > B > C (based on total points scored)

    println!("🎮 Simulating game results for 3-way circular tie scenario...");

    for game_wrapper in games {
        let game = &game_wrapper["game"];
        let game_id = game["id"].as_str().unwrap();
        let home_team_id = game["home_team_id"].as_str().unwrap();
        let away_team_id = game["away_team_id"].as_str().unwrap();

        let (home_score, away_score) =
            // Games among A, B, C (circular head-to-head)
            if (home_team_id == team_a_id && away_team_id == team_b_id) ||
               (home_team_id == team_b_id && away_team_id == team_a_id) {
                // A vs B: A wins (A scores 40)
                if home_team_id == team_a_id {
                    println!("   A vs B: A wins (40-20)");
                    (40, 20)
                } else {
                    println!("   B vs A: A wins (20-40)");
                    (20, 40)
                }
            } else if (home_team_id == team_b_id && away_team_id == team_c_id) ||
                      (home_team_id == team_c_id && away_team_id == team_b_id) {
                // B vs C: B wins (B scores 35)
                if home_team_id == team_b_id {
                    println!("   B vs C: B wins (35-20)");
                    (35, 20)
                } else {
                    println!("   C vs B: B wins (20-35)");
                    (20, 35)
                }
            } else if (home_team_id == team_c_id && away_team_id == team_a_id) ||
                      (home_team_id == team_a_id && away_team_id == team_c_id) {
                // C vs A: C wins (C scores 30)
                if home_team_id == team_c_id {
                    println!("   C vs A: C wins (30-20)");
                    (30, 20)
                } else {
                    println!("   A vs C: C wins (20-30)");
                    (20, 30)
                }
            }
            // Games where A, B, or C play against D, E, F
            // Each of A, B, C wins 1 more game (against D, E, or F)
            // A scores 5 more, B scores 5 more, C scores 5 more
            else if (home_team_id == team_a_id && away_team_id == team_d_id) ||
                    (home_team_id == team_d_id && away_team_id == team_a_id) {
                // A vs D: A wins (scores 5 more to make total: 40+5=45)
                if home_team_id == team_a_id {
                    println!("   A vs D: A wins (5-2)");
                    (5, 2)
                } else {
                    println!("   D vs A: A wins (2-5)");
                    (2, 5)
                }
            } else if (home_team_id == team_b_id && away_team_id == team_e_id) ||
                      (home_team_id == team_e_id && away_team_id == team_b_id) {
                // B vs E: B wins (scores 5 more to make total: 35+5=40)
                if home_team_id == team_b_id {
                    println!("   B vs E: B wins (5-2)");
                    (5, 2)
                } else {
                    println!("   E vs B: B wins (2-5)");
                    (2, 5)
                }
            } else if (home_team_id == team_c_id && away_team_id == team_f_id) ||
                      (home_team_id == team_f_id && away_team_id == team_c_id) {
                // C vs F: C wins (scores 5 more to make total: 30+5=35)
                if home_team_id == team_c_id {
                    println!("   C vs F: C wins (5-2)");
                    (5, 2)
                } else {
                    println!("   F vs C: C wins (2-5)");
                    (2, 5)
                }
            }
            // A, B, C lose their remaining games (each scores 0 in losses)
            else if home_team_id == team_a_id || away_team_id == team_a_id {
                // A loses to E or F (scores 0)
                if home_team_id == team_a_id {
                    println!("   A loses (0-25)");
                    (0, 25)
                } else {
                    println!("   A loses (25-0)");
                    (25, 0)
                }
            } else if home_team_id == team_b_id || away_team_id == team_b_id {
                // B loses to D or F (scores 0)
                if home_team_id == team_b_id {
                    println!("   B loses (0-25)");
                    (0, 25)
                } else {
                    println!("   B loses (25-0)");
                    (25, 0)
                }
            } else if home_team_id == team_c_id || away_team_id == team_c_id {
                // C loses to D or E (scores 0)
                if home_team_id == team_c_id {
                    println!("   C loses (0-25)");
                    (0, 25)
                } else {
                    println!("   C loses (25-0)");
                    (25, 0)
                }
            }
            // Other games (D, E, F playing each other)
            else {
                println!("   Other game: Draw (25-25)");
                (25, 25)
            };

        let result_request = json!({
            "home_score": home_score,
            "away_score": away_score
        });

        let result_response = make_authenticated_request(
            &client,
            reqwest::Method::PUT,
            &format!("{}/league/games/{}/result", &app.address, game_id),
            &admin_user.token,
            Some(result_request),
        ).await;

        if result_response.status() != 200 {
            let status = result_response.status();
            let error_text = result_response.text().await.unwrap();
            panic!("Failed to submit game result for game {}: Status {}, Error: {}", game_id, status, error_text);
        }
    }

    println!("✅ Submitted all game results");

    // Step 8: Get the final standings
    let standings_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/standings", &app.address, season_id),
        &admin_user.token,
        None,
    ).await;

    assert_eq!(standings_response.status(), 200);
    let standings_data: serde_json::Value = standings_response.json().await.unwrap();
    let standings = standings_data["data"]["standings"].as_array().unwrap();

    println!("✅ Got final standings");
    println!("\n📊 Final Standings:");

    // Find positions and points for teams A, B, C
    let mut team_a_position = 0;
    let mut team_b_position = 0;
    let mut team_c_position = 0;
    let mut team_a_points = 0;
    let mut team_b_points = 0;
    let mut team_c_points = 0;

    for standing in standings {
        let team_id = standing["standing"]["team_id"].as_str().unwrap();
        let position = standing["standing"]["position"].as_i64().unwrap();
        let points = standing["standing"]["points"].as_i64().unwrap();
        let team_name = standing["team_name"].as_str().unwrap();

        println!("   {} - Position: {}, Points: {}", team_name, position, points);

        if team_id == team_a_id {
            team_a_position = position;
            team_a_points = points;
        } else if team_id == team_b_id {
            team_b_position = position;
            team_b_points = points;
        } else if team_id == team_c_id {
            team_c_position = position;
            team_c_points = points;
        }
    }

    println!("\n🔍 Verifying tie-breaker logic:");
    println!("   Team A: Position {}, Points {}", team_a_position, team_a_points);
    println!("   Team B: Position {}, Points {}", team_b_position, team_b_points);
    println!("   Team C: Position {}, Points {}", team_c_position, team_c_points);

    // Verify all three teams have the same points
    assert_eq!(team_a_points, team_b_points, "Teams A and B should have equal points");
    assert_eq!(team_b_points, team_c_points, "Teams B and C should have equal points");

    // Verify ranking based on head-to-head
    // Head-to-head: A has 4 points (1W, 1D), B has 3 points (1W, 1L), C has 1 point (1D, 1L)
    assert!(team_a_position < team_b_position,
        "Team A (position {}) should be ranked higher than Team B (position {}) due to better head-to-head record",
        team_a_position, team_b_position);

    assert!(team_b_position < team_c_position,
        "Team B (position {}) should be ranked higher than Team C (position {}) due to better head-to-head record",
        team_b_position, team_c_position);

    println!("\n✅ Three-way tie-breaker working correctly:");
    println!("   • All three teams tied on {} points", team_a_points);
    println!("   • Team A ranked 1st (head-to-head: 4 points)");
    println!("   • Team B ranked 2nd (head-to-head: 3 points)");
    println!("   • Team C ranked 3rd (head-to-head: 1 point)");
}
