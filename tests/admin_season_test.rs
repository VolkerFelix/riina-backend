use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use chrono::{Datelike, Days, NaiveTime, Utc, Weekday};

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request, get_next_date};
use common::admin_helpers::{create_admin_user_and_login, create_teams_for_test};

#[actix_web::test]
async fn admin_generate_schedule_works() {
    // Setup
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    
    // Create admin user and get token
    let admin = create_admin_user_and_login(&app.address).await;

    let league_name = format!("Test League {}", Uuid::new_v4());
    let league_description = "Test League Description";
    let max_teams = 16;

    let league_request = json!({
        "name": league_name,
        "description": league_description,
        "max_teams": max_teams
    });

    let league_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", app.address),
        &admin.token,
        Some(league_request),
    ).await;

    let status = league_response.status().as_u16();
    if status != 201 {
        let text = league_response.text().await.expect("Failed to get response text");
        panic!("Expected 201, got {}: {}", status, text);
    }

    let league: serde_json::Value = league_response.json().await.expect("Failed to parse league response");
    let league_id = league["data"]["id"].as_str().expect("League ID not found");
    
    
    // Create two teams
    let team1_name = format!("Team 1 {}", &Uuid::new_v4().to_string()[..8]);
    let team2_name = format!("Team 2 {}", &Uuid::new_v4().to_string()[..8]);
    let user1 = create_test_user_and_login(&app.address).await;
    let user2 = create_test_user_and_login(&app.address).await;

    let team1_request = json!({
        "name": team1_name,
        "description": "Test Team 1",
        "color": "#FF0000",
        "owner_id": user1.user_id
    });

    let team2_request = json!({
        "name": team2_name,
        "description": "Test Team 2",
        "color": "#00FF00",
        "owner_id": user2.user_id
    });

    let team1_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", app.address),
        &admin.token,
        Some(team1_request),
    ).await;

    let team2_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", app.address),
        &admin.token,
        Some(team2_request),
    ).await;

    assert_eq!(201, team1_response.status().as_u16());
    assert_eq!(201, team2_response.status().as_u16());

    let team1: serde_json::Value = team1_response.json().await.expect("Failed to parse team 1 response");
    let team2: serde_json::Value = team2_response.json().await.expect("Failed to parse team 2 response");
    let team1_id = team1["data"]["id"].as_str().expect("Team 1 ID not found");
    let team2_id = team2["data"]["id"].as_str().expect("Team 2 ID not found");

    let assign_team1_request = json!({
        "team_id": team1_id
    });

    let assign_team2_request = json!({
        "team_id": team2_id
    });
    
    // Assign teams to the league first
    let assign_team1_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/teams", app.address, league_id),
        &admin.token,
        Some(assign_team1_request),
    ).await;

    let assign_team2_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/teams", app.address, league_id),
        &admin.token,
        Some(assign_team2_request),
    ).await;
    
    assert!(assign_team1_response.status().is_success());
    assert!(assign_team2_response.status().is_success());
    
    // Create a season for the league
    let start_date = get_next_date(Weekday::Sat, NaiveTime::from_hms_opt(22, 0, 0).unwrap());

    let season_request = json!({
        "name": "Test Season",
        "start_date": start_date.to_rfc3339()
    });

    let season_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/seasons", app.address, league_id),
        &admin.token,
        Some(season_request),
    ).await;
    
    if !season_response.status().is_success() {
        let status = season_response.status();
        let error_text = season_response.text().await.expect("Failed to get response text");
        panic!("Expected success, got {}: {}", status, error_text);
    }
    let season: serde_json::Value = season_response.json().await.expect("Failed to parse season response");
    let season_id = season["data"]["id"].as_str().expect("Season ID not found");
    
    // Verify games were created in the database
    let games = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM league_games
        WHERE season_id = $1
        "#,
        Uuid::parse_str(&season_id).unwrap()
    )
    .fetch_one(&app.db_pool)
    .await
    .expect("Failed to query games");
    
    assert!(games.count.unwrap() > 0);
}

#[tokio::test]
async fn admin_generate_schedule_with_invalid_date_fails() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a league first
    let league_request = json!({
        "name": format!("Test League {}", Uuid::new_v4()),
        "description": "A test league for schedule generation",
        "max_teams": 4
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", test_app.address),
        &admin.token,
        Some(league_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.unwrap();
    let league_id = create_body["data"]["id"].as_str().unwrap();
    let season_id = league_id; // In the admin system, league and season are the same

    // Create some teams and assign them to the league  
    let team_ids = create_teams_for_test(&test_app.address, &admin.token, 4).await;
    
    // Assign teams to league
    for team_id in &team_ids {
        let assign_request = json!({
            "team_id": team_id
        });

        let assign_response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/leagues/{}/teams", test_app.address, league_id),
            &admin.token,
            Some(assign_request),
        ).await;

        let status = assign_response.status().as_u16();
        if status != 201 {
            let text = assign_response.text().await.expect("Failed to get response text");
            panic!("Expected 201, got {}: {}", status, text);
        }
    }

    // Act - Try to create season with invalid date
    // Create a date that's in the past
    let days = Days::new(2);
    let date = Utc::now().checked_sub_days(days).unwrap();
    //let date = get_next_date(Weekday::Mon, NaiveTime::from_hms_opt(22, 0, 0).unwrap());
    
    let season_request = json!({
        "name": "Test Season with Invalid Date",
        "start_date": date.to_rfc3339()
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/seasons", test_app.address, league_id),
        &admin.token,
        Some(season_request),
    ).await;

    // Assert
    assert_eq!(400, response.status().as_u16());

    let response_text = response.text().await.expect("Failed to get response text");
    if response_text.trim().is_empty() {
        panic!("Expected error response body, got empty body");
    }
    let body: serde_json::Value = serde_json::from_str(&response_text).expect("Failed to parse response");
    
    // Check that the error message indicates Saturday validation failure
    let error_msg = body["error"].as_str().unwrap_or("");
    assert!(error_msg.contains("future"), "Expected future validation error, got: {}", error_msg);
}

#[tokio::test]
async fn test_season_creation_with_proper_round_robin_schedule() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a league
    let league_request = json!({
        "name": "Test League for Round Robin",
        "description": "Testing proper schedule generation",
        "max_teams": 6
    });

    let league_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", test_app.address),
        &admin.token,
        Some(league_request),
    ).await;

    assert_eq!(201, league_response.status().as_u16());
    let league_body: serde_json::Value = league_response.json().await.expect("Failed to parse league response");
    let league_id = league_body["data"]["id"].as_str().expect("League ID not found").to_string();

    // Create exactly 6 teams (even number)
    let team_ids = create_teams_for_test(&test_app.address, &admin.token, 6).await;
    
    // Assign teams to league
    for team_id in &team_ids {
        let assign_request = json!({
            "team_id": team_id
        });

        let assign_response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/leagues/{}/teams", test_app.address, league_id),
            &admin.token,
            Some(assign_request),
        ).await;

        assert_eq!(201, assign_response.status().as_u16());
    }

    // Create season with proper schedule generation
    let start_date = get_next_date(Weekday::Sat, NaiveTime::from_hms_opt(22, 0, 0).unwrap());
    
    let season_request = json!({
        "name": "Test Season",
        "start_date": start_date.to_rfc3339()
    });

    let season_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/seasons", test_app.address, league_id),
        &admin.token,
        Some(season_request),
    ).await;

    assert_eq!(201, season_response.status().as_u16());
    let season_body: serde_json::Value = season_response.json().await.expect("Failed to parse season response");
    let season_id = season_body["data"]["id"].as_str().expect("Season ID not found").to_string();

    // Verify schedule was generated correctly
    let schedule_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/seasons/{}/schedule", test_app.address, season_id),
        &admin.token,
        None,
    ).await;

    assert_eq!(200, schedule_response.status().as_u16());
    let schedule_body: serde_json::Value = schedule_response.json().await.expect("Failed to parse schedule response");
    
    let games = schedule_body["data"]["games"].as_array().expect("Games not found");
    let total_weeks = schedule_body["data"]["total_weeks"].as_i64().expect("Total weeks not found");

    // **VERIFY ROUND-ROBIN PROPERTIES**
    
    // 1. Total games: 6 teams = 6*(6-1) = 30 total games
    assert_eq!(30, games.len(), "Should have exactly 30 games for 6 teams");
    
    // 2. Total weeks: 2*(N-1) = 2*(6-1) = 10 weeks
    assert_eq!(10, total_weeks, "Should have exactly 10 weeks for 6 teams");
    
    // 3. Games per week: exactly 3 games each week
    let mut games_by_week: HashMap<i64, Vec<&serde_json::Value>> = HashMap::new();
    for game in games {
        let week = game["game"]["week_number"].as_i64().expect("Week number not found");
        games_by_week.entry(week).or_insert_with(Vec::new).push(game);
    }
    
    // Verify exactly 10 weeks exist
    assert_eq!(10, games_by_week.len(), "Should have exactly 10 weeks");
    
    // Verify exactly 3 games per week
    for week in 1..=10 {
        let week_games = games_by_week.get(&week).expect(&format!("Week {} not found", week));
        assert_eq!(3, week_games.len(), "Week {} should have exactly 3 games", week);
    }
    
    // 4. No team conflicts: each team plays exactly once per week
    for week in 1..=10 {
        let week_games = games_by_week.get(&week).unwrap();
        let mut teams_this_week = HashSet::new();
        
        for game in week_games {
            let home_team = game["game"]["home_team_id"].as_str().expect("Home team ID not found");
            let away_team = game["game"]["away_team_id"].as_str().expect("Away team ID not found");
            
            assert!(!teams_this_week.contains(home_team), 
                   "Team {} plays multiple games in week {}", home_team, week);
            assert!(!teams_this_week.contains(away_team), 
                   "Team {} plays multiple games in week {}", away_team, week);
            
            teams_this_week.insert(home_team);
            teams_this_week.insert(away_team);
        }
        
        // All 6 teams should be playing in each week
        assert_eq!(6, teams_this_week.len(), "All 6 teams should play in week {}", week);
    }
    
    // 5. Each team plays every other team exactly twice (home and away)
    let mut matchup_count: HashMap<(String, String), i32> = HashMap::new();
    
    for game in games {
        let home_team = game["game"]["home_team_id"].as_str().expect("Home team ID not found").to_string();
        let away_team = game["game"]["away_team_id"].as_str().expect("Away team ID not found").to_string();
        
        // Create sorted pair to represent the matchup regardless of home/away
        let pair = if home_team < away_team {
            (home_team, away_team)
        } else {
            (away_team, home_team)
        };
        
        *matchup_count.entry(pair).or_insert(0) += 1;
    }
    
    // Should have exactly 15 unique matchups (6 choose 2 = 15)
    assert_eq!(15, matchup_count.len(), "Should have exactly 15 unique matchups");
    
    // Each matchup should occur exactly twice (home and away)
    for (pair, count) in matchup_count {
        assert_eq!(2, count, "Teams {:?} should play exactly twice", pair);
    }
    
    // 6. Verify first leg vs second leg distribution
    let first_leg_games: Vec<_> = games.iter()
        .filter(|g| g["game"]["is_first_leg"].as_bool().unwrap_or(false))
        .collect();
    let second_leg_games: Vec<_> = games.iter()
        .filter(|g| !g["game"]["is_first_leg"].as_bool().unwrap_or(true))
        .collect();
    
    assert_eq!(15, first_leg_games.len(), "Should have exactly 15 first leg games");
    assert_eq!(15, second_leg_games.len(), "Should have exactly 15 second leg games");
    
    // 7. Verify home/away balance: each team should have balanced home/away games
    let mut home_games_count: HashMap<String, i32> = HashMap::new();
    let mut away_games_count: HashMap<String, i32> = HashMap::new();
    
    for game in games {
        let home_team = game["game"]["home_team_id"].as_str().expect("Home team ID not found").to_string();
        let away_team = game["game"]["away_team_id"].as_str().expect("Away team ID not found").to_string();
        
        *home_games_count.entry(home_team).or_insert(0) += 1;
        *away_games_count.entry(away_team).or_insert(0) += 1;
    }
    
    // Each team should play exactly 5 home games and 5 away games
    for team_id in &team_ids {
        let home_count = home_games_count.get(team_id).unwrap_or(&0);
        let away_count = away_games_count.get(team_id).unwrap_or(&0);
        
        assert_eq!(5, *home_count, "Team {} should have exactly 5 home games", team_id);
        assert_eq!(5, *away_count, "Team {} should have exactly 5 away games", team_id);
    }

    println!("✅ Season creation test passed - Perfect round-robin schedule generated!");
}