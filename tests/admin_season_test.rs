use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;
use bcrypt;
use std::collections::{HashMap, HashSet};
use chrono::{Datelike, Timelike};

mod common;
use common::utils::{spawn_app, TestApp};
use common::admin_helpers::{create_test_user_and_login, create_test_user_and_login_with_id, make_authenticated_request, create_teams_for_test};

/// Helper function to create a test admin user
async fn create_test_admin(pool: &PgPool) -> (String, String, Uuid) {
    let username = format!("adminuser{}", Uuid::new_v4());
    let email = format!("{}@example.com", username);
    let password = "password123";
    let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).expect("Failed to hash password");

    // Create user
    let user = sqlx::query!(
        "INSERT INTO users (username, email, password_hash, role) VALUES ($1, $2, $3, 'admin') RETURNING id",
        username,
        email,
        password_hash
    )
    .fetch_one(pool)
    .await
    .expect("Failed to create test admin");

    (username, email, user.id)
}

/// Helper function to login and get token
async fn login_and_get_token(client: &Client, app_address: &str, email: &str, password: &str) -> String {
    let username = email.split('@').next().unwrap();
    let login_request = json!({
        "username": username,
        "password": password
    });

    let login_response = client
        .post(&format!("{}/login", app_address))
        .json(&login_request)
        .send()
        .await
        .expect("Failed to login");

    assert_eq!(200, login_response.status().as_u16());

    let login_body: serde_json::Value = login_response
        .json()
        .await
        .expect("Failed to parse login response");

    login_body["token"].as_str().unwrap().to_string()
}

#[actix_web::test]
async fn admin_generate_schedule_works() {
    // Setup
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    
    // Create admin users and get tokens
    let (username1, email1, user_id1) = create_test_admin(&app.db_pool).await;
    let (username2, email2, user_id2) = create_test_admin(&app.db_pool).await;
    let token = login_and_get_token(&client, &app.address, &email1, "password123").await;
    let team1_name = format!("Team 1 {}", Uuid::new_v4());
    let team2_name = format!("Team 2 {}", Uuid::new_v4());
    // Create a league first
    let league_response = client
        .post(&format!("{}/admin/leagues", app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "name": format!("Test League {}", Uuid::new_v4()),
            "description": "Test League Description",
            "max_teams": 16
        }))
        .send()
        .await
        .expect("Failed to create league");
    
    assert!(league_response.status().is_success());
    let league: serde_json::Value = league_response.json().await.expect("Failed to parse league response");
    let league_id = league["data"]["id"].as_str().expect("League ID not found");
    
    // Create two teams
    let team1_response = client
        .post(&format!("{}/admin/teams", app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "name": team1_name,
            "description": "Test Team 1",
            "color": "#FF0000",
            "formation": "circle",
            "owner_id": user_id1
        }))
        .send()
        .await
        .expect("Failed to create team 1");
    
    let team2_response = client
        .post(&format!("{}/admin/teams", app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "name": team2_name,
            "description": "Test Team 2",
            "color": "#00FF00",
            "formation": "line",
            "owner_id": user_id2
        }))
        .send()
        .await
        .expect("Failed to create team 2");
    
    assert!(team1_response.status().is_success());
    assert!(team2_response.status().is_success());
    
    let team1: serde_json::Value = team1_response.json().await.expect("Failed to parse team 1 response");
    let team2: serde_json::Value = team2_response.json().await.expect("Failed to parse team 2 response");
    let team1_id = team1["data"]["id"].as_str().expect("Team 1 ID not found");
    let team2_id = team2["data"]["id"].as_str().expect("Team 2 ID not found");
    
    // Assign teams to the league first
    let assign_team1_response = client
        .post(&format!("{}/admin/leagues/{}/teams", app.address, league_id))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "team_id": team1_id
        }))
        .send()
        .await
        .expect("Failed to assign team 1");
    
    let assign_team2_response = client
        .post(&format!("{}/admin/leagues/{}/teams", app.address, league_id))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "team_id": team2_id
        }))
        .send()
        .await
        .expect("Failed to assign team 2");
    
    assert!(assign_team1_response.status().is_success());
    assert!(assign_team2_response.status().is_success());
    
    // Create a season for the league
    let season_response = client
        .post(&format!("{}/admin/leagues/{}/seasons", app.address, league_id))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "name": "Test Season 2026",
            "start_date": "2026-06-20T22:00:00Z"
        }))
        .send()
        .await
        .expect("Failed to create season");
    
    assert!(season_response.status().is_success());
    let season: serde_json::Value = season_response.json().await.expect("Failed to parse season response");
    let season_id = season["data"]["id"].as_str().expect("Season ID not found");
    
    // Generate schedule
    let response = client
        .post(&format!("{}/admin/leagues/{}/schedule", app.address, league_id))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "season_id": season_id,
            "start_date": "2026-06-20T22:00:00Z"  // Saturday at 22:00 UTC (future date)
        }))
        .send()
        .await
        .expect("Failed to execute request");
    
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.expect("Failed to get response text");
        panic!("Expected success, got {}: {}", status, text);
    }
    
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["success"].as_bool().unwrap());
    assert!(response_body["data"]["games_created"].as_i64().unwrap() > 0);
    
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
    let token = create_test_user_and_login(&test_app.address).await;

    // Create a league first
    let league_request = json!({
        "name": format!("Test League {}", Uuid::new_v4()),
        "description": "A test league for schedule generation",
        "max_teams": 4,
        "season_start_date": "2024-03-23T22:00:00Z",
        "season_end_date": "2024-12-31T23:59:59Z"
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", test_app.address),
        &token,
        Some(league_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.unwrap();
    let league_id = create_body["data"]["id"].as_str().unwrap();
    let season_id = league_id; // In the admin system, league and season are the same

    // Create some teams and assign them to the league  
    let team_ids = create_teams_for_test(&test_app.address, &token, 4).await;
    
    // Assign teams to league
    for team_id in &team_ids {
        let assign_request = json!({
            "team_id": team_id
        });

        let assign_response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/leagues/{}/teams", test_app.address, league_id),
            &token,
            Some(assign_request),
        ).await;

        let status = assign_response.status().as_u16();
        if status != 201 {
            let text = assign_response.text().await.expect("Failed to get response text");
            panic!("Expected 201, got {}: {}", status, text);
        }
    }

    // Act - Try to generate schedule with invalid date (not a Saturday)
    let schedule_request = json!({
        "season_id": season_id,
        "start_date": "2024-03-24T22:00:00Z" // Sunday at 22:00 UTC
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/schedule", test_app.address, league_id),
        &token,
        Some(schedule_request),
    ).await;

    // Assert
    assert_eq!(400, response.status().as_u16());

    let response_text = response.text().await.expect("Failed to get response text");
    if response_text.trim().is_empty() {
        panic!("Expected error response body, got empty body");
    }
    let body: serde_json::Value = serde_json::from_str(&response_text).expect("Failed to parse response");
    assert!(!body["success"].as_bool().unwrap_or(false));
    assert!(body["message"].as_str().unwrap_or("").contains("Start date must be a Saturday"));
}

#[tokio::test]
async fn test_season_creation_with_proper_round_robin_schedule() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

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
        &token,
        Some(league_request),
    ).await;

    assert_eq!(201, league_response.status().as_u16());
    let league_body: serde_json::Value = league_response.json().await.expect("Failed to parse league response");
    let league_id = league_body["data"]["id"].as_str().expect("League ID not found").to_string();

    // Create exactly 6 teams (even number)
    let team_ids = create_teams_for_test(&test_app.address, &token, 6).await;
    
    // Assign teams to league
    for team_id in &team_ids {
        let assign_request = json!({
            "team_id": team_id
        });

        let assign_response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/leagues/{}/teams", test_app.address, league_id),
            &token,
            Some(assign_request),
        ).await;

        assert_eq!(201, assign_response.status().as_u16());
    }

    // Create season with proper schedule generation
    let now = chrono::Utc::now();
    let days_until_saturday = (6 - now.weekday().num_days_from_monday()) % 7;
    let next_saturday = if days_until_saturday == 0 && now.hour() >= 22 {
        now + chrono::Duration::days(7) // If it's Saturday after 22:00, get next Saturday
    } else if days_until_saturday == 0 {
        now // If it's Saturday before 22:00, use today
    } else {
        now + chrono::Duration::days(days_until_saturday as i64)
    };
    let start_date = next_saturday
        .with_hour(22)
        .unwrap()
        .with_minute(0)
        .unwrap()
        .with_second(0)
        .unwrap()
        .format("%Y-%m-%dT%H:%M:%SZ");
    
    let season_request = json!({
        "name": "Test Season",
        "start_date": start_date.to_string()
    });

    let season_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/seasons", test_app.address, league_id),
        &token,
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
        &token,
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