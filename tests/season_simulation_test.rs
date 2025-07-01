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
};
use common::admin_helpers::{create_admin_user_and_login, create_league_season};
use common::health_data_helpers::{
    create_beginner_health_data, 
    create_intermediate_health_data, 
    create_advanced_health_data, 
    create_elite_health_data,
    upload_health_data_for_user
};
use serde_json::json;
use uuid::Uuid;
use rand::prelude::*;
use reqwest::Client;
use chrono::{Weekday, NaiveTime};

#[tokio::test]
async fn simulate_complete_season_with_4_players_2_teams() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🏟️ Starting season simulation with 4 players and 2 teams");
    
    // Step 1: Create 4 users (2 team owners + 2 additional players)
    let admin_user = create_admin_user_and_login(&app.address).await;
    let user1 = create_test_user_and_login(&app.address).await;
    let user2 = create_test_user_and_login(&app.address).await;
    let user3 = create_test_user_and_login(&app.address).await;
    let user4 = create_test_user_and_login(&app.address).await;
    
    println!("✅ Created 5 users (1 admin + 4 players)");
    
    // Step 2: Create a league
    let league_request = json!({
        "name": "Simulation League",
        "description": "League for season simulation with 4 players",
        "max_teams": 2
    });
    
    let league_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", &app.address),
        &admin_user.token,
        Some(league_request),
    ).await;
    
    let status = league_response.status();
    
    if status != 201 {
        let error_text = league_response.text().await.unwrap();
        panic!("Failed to create league. Status: {}, Body: {}", status, error_text);
    }
    
    let league_data: serde_json::Value = league_response.json().await.unwrap();
    let league_id = league_data["data"]["id"].as_str().unwrap();
    
    println!("✅ Created league: {}", league_id);
    
    // Step 3: Create 2 teams
    let team1_request = json!({
        "name": format!("Fire Dragons {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#DC2626",
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
        "name": format!("Ice Warriors {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#2563EB",
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
    
    println!("✅ Created teams: {} (Fire Dragons), {} (Ice Warriors)", team1_id, team2_id);
    
    // Step 4: Add second player to each team (team members)
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
    
    println!("✅ Added team members: User2 → Fire Dragons, User4 → Ice Warriors");
    
    // Step 5: Assign teams to league
    let assign_team1_request = json!({
        "team_id": team1_id
    });
    
    let assign1_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/teams", &app.address, league_id),
        &admin_user.token,
        Some(assign_team1_request),
    ).await;
    
    assert_eq!(assign1_response.status(), 201);
    
    let assign_team2_request = json!({
        "team_id": team2_id
    });
    
    let assign2_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/teams", &app.address, league_id),
        &admin_user.token,
        Some(assign_team2_request),
    ).await;
    
    assert_eq!(assign2_response.status(), 201);
    
    println!("✅ Assigned both teams to league");
    
    // Step 6: Create a season
    let start_date = get_next_date(Weekday::Sun, NaiveTime::from_hms_opt(10, 0, 0).unwrap());
    let season_id = create_league_season(
        &app.address,
        &admin_user.token,
        league_id,
        "Simulation Season 2025",
        &start_date.to_rfc3339()
    ).await;
    
    println!("✅ Created season: {}", season_id);
    
    // Step 7: Upload health data for different fitness levels
    println!("🏃 Uploading health data to build user stats...");
    
    // User1 (Team1 Owner): Elite athlete
    let user1_health = create_elite_health_data();
    let user1_upload = upload_health_data_for_user(&client, &app.address, &user1.token, user1_health).await;
    match user1_upload {
        Ok(response) => {
            println!("   ✅ User1 (Fire Dragons Owner): Elite fitness data uploaded");
            println!("   📊 User1 upload response: {}", serde_json::to_string_pretty(&response).unwrap_or("failed to format".to_string()));
        }
        Err(e) => panic!("Failed to upload health data for user1: {}", e),
    }
    
    // User2 (Team1 Member): Advanced athlete  
    let user2_health = create_advanced_health_data();
    let user2_upload = upload_health_data_for_user(&client, &app.address, &user2.token, user2_health).await;
    match user2_upload {
        Ok(response) => {
            println!("   ✅ User2 (Fire Dragons Member): Advanced fitness data uploaded");
            println!("   📊 User2 upload response: {}", serde_json::to_string_pretty(&response).unwrap_or("failed to format".to_string()));
        }
        Err(e) => panic!("Failed to upload health data for user2: {}", e),
    }
    
    // User3 (Team2 Owner): Intermediate athlete
    let user3_health = create_intermediate_health_data();
    let user3_upload = upload_health_data_for_user(&client, &app.address, &user3.token, user3_health).await;
    if let Err(e) = user3_upload {
        panic!("Failed to upload health data for user3: {}", e);
    }
    println!("   ✅ User3 (Ice Warriors Owner): Intermediate fitness data uploaded");
    
    // User4 (Team2 Member): Beginner athlete
    let user4_health = create_beginner_health_data();
    let user4_upload = upload_health_data_for_user(&client, &app.address, &user4.token, user4_health).await;
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
            println!("   ⚠️  Failed to get user stats for {}: {}", user_id, response.status());
            return (50, 50, 100); // Default values
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
    
    // Step 11: Get final standings
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
    
    // Step 12: Verify season integrity
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