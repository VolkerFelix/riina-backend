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

/// Configuration for creating a team
pub struct TeamConfig {
    pub name: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub owner_id: Option<Uuid>,
}

impl Default for TeamConfig {
    fn default() -> Self {
        Self {
            name: None,
            color: None,
            description: None,
            owner_id: None,
        }
    }
}

/// Helper function to create a single team with optional configuration
pub async fn create_team(
    app_address: &str,
    admin_token: &str,
    config: TeamConfig,
) -> String {
    let client = Client::new();
    let unique_suffix = &Uuid::new_v4().to_string()[..8];
    
    let team_name = config.name.unwrap_or_else(|| format!("Test Team {}", unique_suffix));
    let team_color = config.color.unwrap_or_else(|| format!("#{:06X}", Uuid::new_v4().as_u128() as u32 % 0xFFFFFF));
    
    let mut team_request = json!({
        "name": team_name,
        "color": team_color,
    });
    
    if let Some(description) = config.description {
        team_request["description"] = json!(description);
    }
    
    if let Some(owner_id) = config.owner_id {
        team_request["owner_id"] = json!(owner_id.to_string());
    }

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", app_address),
        admin_token,
        Some(team_request),
    ).await;

    assert_eq!(response.status(), 201, "Failed to create team");
    let team_data: serde_json::Value = response.json().await.expect("Failed to parse team response");
    team_data["data"]["id"].as_str().expect("Team ID not found").to_string()
}

/// Result structure for league and teams setup
pub struct LeagueWithTeamsResult {
    pub league_id: String,
    pub team_ids: Vec<String>,
}

/// Helper function to create a league with teams and optionally add teams to the league.
/// This is a common pattern in tests and reduces code duplication.
///
/// # Arguments
/// * `app_address` - The application address
/// * `admin_token` - Admin authentication token
/// * `max_teams` - Maximum number of teams for the league
/// * `team_count` - Number of teams to create
/// * `team_owners` - Optional vector of user IDs to use as team owners. If None or shorter than team_count, 
///                   new users will be created for the remaining teams.
/// * `add_to_league` - Whether to automatically add teams to the league
/// * `league_name` - Optional custom league name
/// * `league_description` - Optional custom league description
pub async fn create_league_with_teams(
    app_address: &str,
    admin_token: &str,
    max_teams: i32,
    team_count: usize,
    team_owners: Option<Vec<Uuid>>,
    add_to_league: bool,
    league_name: Option<String>,
    league_description: Option<String>,
) -> LeagueWithTeamsResult {
    let client = Client::new();
    
    // Create league
    let league_name_value = league_name.unwrap_or_else(|| format!("Test League {}", &Uuid::new_v4().to_string()[..4]));
    let league_description_value = league_description.unwrap_or_else(|| "Test league for integration tests".to_string());
    
    let league_request = json!({
        "name": league_name_value,
        "description": league_description_value,
        "max_teams": max_teams
    });

    let league_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", app_address),
        admin_token,
        Some(league_request),
    ).await;

    assert_eq!(league_response.status(), 201, "Failed to create league");
    let league_data: serde_json::Value = league_response.json().await.expect("Failed to parse league response");
    let league_id = league_data["data"]["id"].as_str().expect("League ID not found").to_string();

    // Prepare team owners
    let mut owner_ids = team_owners.unwrap_or_default();
    
    // If we need more owners than provided, create additional users
    if owner_ids.len() < team_count {
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");
        let pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to database");
        
        for i in owner_ids.len()..team_count {
            let username = format!("teamowner{}{}", i, Uuid::new_v4());
            let password = "password123";
            let email = format!("{}@example.com", username);

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
            
            let user_record = sqlx::query!(
                "SELECT id FROM users WHERE username = $1",
                username
            )
            .fetch_one(&pool)
            .await
            .expect("Failed to get user ID");
            
            owner_ids.push(user_record.id);
        }
    }

    // Create teams
    let mut team_ids = Vec::new();
    let colors = vec!["#DC2626", "#2563EB", "#16A34A", "#CA8A04", "#9333EA", "#EC4899", "#06B6D4", "#F59E0B"];
    
    for i in 0..team_count {
        let team_name = format!("Test Team {} {}", i + 1, &Uuid::new_v4().to_string()[..8]);
        let team_color = colors.get(i % colors.len()).unwrap_or(&"#000000");
        
        let team_request = json!({
            "name": team_name,
            "color": team_color,
            "owner_id": owner_ids[i].to_string()
        });

        let team_response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/teams", app_address),
            admin_token,
            Some(team_request),
        ).await;

        assert_eq!(team_response.status(), 201, "Failed to create team");
        let team_data: serde_json::Value = team_response.json().await.expect("Failed to parse team response");
        let team_id = team_data["data"]["id"].as_str().expect("Team ID not found").to_string();
        team_ids.push(team_id);
    }

    // Optionally add teams to league
    if add_to_league {
        for team_id in &team_ids {
            add_team_to_league(app_address, admin_token, &league_id, team_id).await;
        }
    }

    LeagueWithTeamsResult {
        league_id,
        team_ids,
    }
}