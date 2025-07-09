use evolveme_backend::league::league;
use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;

use crate::common::utils::{
    UserRegLoginResponse,
    parse_user_id_from_jwt_token,
    make_authenticated_request
};

/// Helper function to create an admin user and get auth token
pub async fn create_admin_user_and_login(app_address: &str) -> UserRegLoginResponse {
    let client = Client::new();
    let username = format!("adminuser{}", Uuid::new_v4());
    let password = "password123";
    let email = format!("{}@example.com", username);

    // Register user
    let user_request = json!({
        "username": username,
        "password": password,
        "email": email
    });

    let register_response = client
        .post(&format!("{}/register_user", app_address))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to register user");

    assert_eq!(200, register_response.status().as_u16());

    // Promote user to admin role using direct database access
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");
    
    sqlx::query!(
        "UPDATE users SET role = 'admin' WHERE username = $1",
        username
    )
    .execute(&pool)
    .await
    .expect("Failed to promote user to admin");

    // Login and get token
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

    let token = login_body["token"].as_str().unwrap().to_string();
    let user_id = parse_user_id_from_jwt_token(&token);

    UserRegLoginResponse {
        token,
        user_id,
        username
    }
}

/// Helper function to create a league
pub async fn create_league(app_address: &str, token: &str, amount_of_teams: i32) -> String {
    let client = Client::new();
    let league_name = format!("Test League {}", &Uuid::new_v4().to_string()[..4]);
    let league_request = json!({
        "name": league_name,
        "description": "Testing live game service",
        "max_teams": amount_of_teams
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", app_address),
        token,
        Some(league_request)
    ).await;

    assert_eq!(response.status(), 201);
    let league_data: serde_json::Value = response.json().await.unwrap();
    league_data["data"]["id"].as_str().unwrap().to_string()
}

/// Helper function to create a league season
pub async fn create_league_season(
    app_address: &str,
    token: &str,
    league_id: &str,
    season_name: &str,
    start_date: &str,
) -> String {
    let client = Client::new();
    
    let season_request = json!({
        "name": season_name,
        "start_date": start_date,
        "evaluation_cron": "0 0 22 * * SAT" // Default: Saturday 10 PM UTC
    });

    let season_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/seasons", app_address, league_id),
        token,
        Some(season_request),
    ).await;

    let status = season_response.status();
    let response_text = season_response.text().await.expect("Failed to read response");
    
    if status != 201 {
        panic!("Failed to create season. Status: {}. Body: {}", status, response_text);
    }
    
    let season_data: serde_json::Value = serde_json::from_str(&response_text).expect("Failed to parse season response");
    season_data["data"]["id"].as_str().expect("Season ID not found").to_string()
}

/// Helper function to create a league season with evaluation schedule
pub async fn create_league_season_with_schedule(
    app_address: &str,
    token: &str,
    league_id: &str,
    season_name: &str,
    start_date: &str,
    evaluation_cron: &str,
    evaluation_timezone: Option<&str>,
    auto_evaluation_enabled: Option<bool>,
) -> String {
    let client = Client::new();
    
    let mut season_request = json!({
        "name": season_name,
        "start_date": start_date,
        "evaluation_cron": evaluation_cron
    });
    if let Some(tz) = evaluation_timezone {
        season_request["evaluation_timezone"] = json!(tz);
    }
    if let Some(enabled) = auto_evaluation_enabled {
        season_request["auto_evaluation_enabled"] = json!(enabled);
    }

    let season_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/seasons", app_address, league_id),
        token,
        Some(season_request),
    ).await;

    assert_eq!(season_response.status(), 201, "Failed to create season with schedule");
    let season_data: serde_json::Value = season_response.json().await.expect("Failed to parse season response");
    season_data["data"]["id"].as_str().expect("Season ID not found").to_string()
}

/// Helper function to create teams for testing
pub async fn create_teams_for_test(app_address: &str, token: &str, count: usize) -> Vec<String> {
    let client = Client::new();
    let mut team_ids = Vec::new();

    // Create regular users first to use as team owners
    let mut user_ids = Vec::new();
    for i in 0..count {
        let username = format!("teamowner{}{}", i, Uuid::new_v4());
        let password = "password123";
        let email = format!("{}@example.com", username);

        // Create user
        let user_request = json!({
            "username": username,
            "password": password,
            "email": email
        });

        let user_response = client
            .post(&format!("{}/register_user", app_address))
            .json(&user_request)
            .send()
            .await
            .expect("Failed to register user");

        assert_eq!(200, user_response.status().as_u16());
        
        // Get user ID from database query
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");
        let pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to database");
        
        let user_record = sqlx::query!(
            "SELECT id FROM users WHERE username = $1",
            username
        )
        .fetch_one(&pool)
        .await
        .expect("Failed to get user ID");
        
        user_ids.push(user_record.id.to_string());
    }

    // Now create teams with the user IDs as owners
    for i in 0..count {
        let team_request = json!({
            "name": format!("Test Team {} {}", i + 1, &Uuid::new_v4().to_string()[..8]),
            "color": format!("#{:06X}", (i * 0x111111) % 0xFFFFFF),
            "owner_id": user_ids[i]
        });

        let response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/teams", app_address),
            token,
            Some(team_request)
        ).await;

        assert_eq!(201, response.status().as_u16());
        let body: serde_json::Value = response.json().await.expect("Failed to parse response");
        let team_id = body["data"]["id"].as_str().expect("Team ID not found").to_string();
        team_ids.push(team_id);
    }

    team_ids
}

pub async fn add_team_to_league(app_address: &str, admin_token: &str, league_id: &str, team_id: &str) {
    let client = Client::new();
    let team_data = json!({
        "team_id": team_id,
        "league_id": league_id
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/teams", app_address, league_id),
        admin_token,
        Some(team_data),
    ).await;

    assert!(response.status().is_success());
}

pub async fn add_user_to_team(app_address: &str, admin_token: &str, team_id: &str, user_id: Uuid) {
    let client = Client::new();
    let member_data = json!({
        "user_id": user_id,
        "role": "member"
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams/{}/members", app_address, team_id),
        admin_token,
        Some(member_data),
    ).await;

    assert!(response.status().is_success());
}