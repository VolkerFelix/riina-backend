use reqwest::{Client, Response};
use serde_json::json;
use uuid::Uuid;

mod common;
use common::utils::spawn_app;

/// Helper function to create a test user and get auth token
async fn create_test_user_and_login(app_address: &str) -> String {
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

    login_body["token"].as_str().unwrap().to_string()
}

/// Helper function to create test users
async fn create_multiple_test_users(app_address: &str, count: usize) -> Vec<String> {
    let client = Client::new();
    let mut user_ids = Vec::new();

    for i in 0..count {
        let username = format!("testuser{}{}", i, Uuid::new_v4());
        let password = "password123";
        let email = format!("{}@example.com", username);

        let user_request = json!({
            "username": username,
            "password": password,
            "email": email
        });

        let response = client
            .post(&format!("{}/register_user", app_address))
            .json(&user_request)
            .send()
            .await
            .expect("Failed to register user");

        assert_eq!(200, response.status().as_u16());
        user_ids.push(username);
    }

    user_ids
}

/// Helper function to create authenticated request
async fn make_authenticated_request(
    client: &Client,
    method: reqwest::Method,
    url: &str,
    token: &str,
    body: Option<serde_json::Value>,
) -> Response {
    let mut request = client
        .request(method, url)
        .header("Authorization", format!("Bearer {}", token));

    if let Some(body) = body {
        request = request.json(&body);
    }

    request.send().await.expect("Failed to send request")
}

#[tokio::test]
async fn admin_routes_require_authentication() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_routes = vec![
        ("GET", "/admin/users"),
        ("GET", "/admin/teams"),
        ("GET", "/admin/leagues"),
        ("GET", "/admin/users/without-team"),
    ];

    // Act & Assert
    for (method, route) in test_routes {
        let response = client
            .request(method.parse().unwrap(), &format!("{}{}", test_app.address, route))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(
            401,
            response.status().as_u16(),
            "Route {} should require authentication",
            route
        );
    }
}

#[tokio::test]
async fn admin_get_users_returns_paginated_results() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;
    
    // Create additional test users
    create_multiple_test_users(&test_app.address, 5).await;

    // Act
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users", test_app.address),
        &token,
        None,
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["data"].is_array());
    assert!(body["pagination"].is_object());
    assert!(body["pagination"]["total"].as_i64().unwrap() >= 6); // 1 admin + 5 test users
    assert_eq!(body["pagination"]["page"].as_i64().unwrap(), 1);
    assert_eq!(body["pagination"]["limit"].as_i64().unwrap(), 20);
}

#[tokio::test]
async fn admin_get_users_supports_pagination() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;
    
    // Create test users
    create_multiple_test_users(&test_app.address, 3).await;

    // Act - Test with limit=2
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users?limit=2&page=1", test_app.address),
        &token,
        None,
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert_eq!(body["data"].as_array().unwrap().len(), 2);
    assert_eq!(body["pagination"]["limit"].as_i64().unwrap(), 2);
    assert!(body["pagination"]["total_pages"].as_i64().unwrap() >= 2);
}

#[tokio::test]
async fn admin_get_users_supports_search() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;
    
    // Create a user with specific username
    let search_username = format!("searchableuser{}", Uuid::new_v4());
    let client_temp = Client::new();
    let user_request = json!({
        "username": search_username,
        "password": "password123",
        "email": format!("{}@example.com", search_username)
    });

    let register_response = client_temp
        .post(&format!("{}/register_user", test_app.address))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to register searchable user");

    assert_eq!(200, register_response.status().as_u16());

    // Act - Search for the specific user
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users?search=searchableuser", test_app.address),
        &token,
        None,
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    let users = body["data"].as_array().unwrap();
    
    assert!(users.len() >= 1);
    assert!(users.iter().any(|user| 
        user["username"].as_str().unwrap().contains("searchableuser")
    ));
}

#[tokio::test]
async fn admin_get_users_without_team_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;
    
    // Create some test users (they should all be without teams initially)
    create_multiple_test_users(&test_app.address, 3).await;

    // Act
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users/without-team", test_app.address),
        &token,
        None,
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["data"].is_array());
    assert!(body["success"].as_bool().unwrap());
    assert!(body["data"].as_array().unwrap().len() >= 4); // admin + 3 test users
}

#[tokio::test]
async fn admin_get_user_by_id_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // First get list of users to get a valid user ID
    let users_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users?limit=1", test_app.address),
        &token,
        None,
    ).await;

    let users_body: serde_json::Value = users_response.json().await.unwrap();
    let user_id = users_body["data"][0]["id"].as_str().unwrap();

    // Act
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users/{}", test_app.address, user_id),
        &token,
        None,
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["data"].is_object());
    assert!(body["success"].as_bool().unwrap());
    assert_eq!(body["data"]["id"].as_str().unwrap(), user_id);
}

#[tokio::test]
async fn admin_get_user_by_invalid_id_returns_404() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;
    let invalid_id = Uuid::new_v4();

    // Act
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users/{}", test_app.address, invalid_id),
        &token,
        None,
    ).await;

    // Assert
    assert_eq!(404, response.status().as_u16());
}

#[tokio::test]
async fn admin_update_user_status_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Get a user ID
    let users_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users?limit=1", test_app.address),
        &token,
        None,
    ).await;

    let users_body: serde_json::Value = users_response.json().await.unwrap();
    let user_id = users_body["data"][0]["id"].as_str().unwrap();

    // Act
    let update_request = json!({
        "status": "inactive"
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::PATCH,
        &format!("{}/admin/users/{}/status", test_app.address, user_id),
        &token,
        Some(update_request),
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["success"].as_bool().unwrap());
    assert_eq!(body["data"]["status"].as_str().unwrap(), "inactive");
}

#[tokio::test]
async fn admin_get_teams_returns_paginated_results() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Act
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/teams", test_app.address),
        &token,
        None,
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["data"].is_array());
    assert!(body["pagination"].is_object());
    assert_eq!(body["pagination"]["page"].as_i64().unwrap(), 1);
    assert_eq!(body["pagination"]["limit"].as_i64().unwrap(), 20);
}

#[tokio::test]
async fn admin_create_team_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Act
    let team_request = json!({
        "name": format!("Team{}", Uuid::new_v4().to_string()[..8].to_string()),
        "color": "#FF0000",
        "formation": "circle"
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &token,
        Some(team_request.clone()),
    ).await;

    // Assert
    assert_eq!(201, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["success"].as_bool().unwrap());
    assert!(body["data"]["id"].is_string());
    assert_eq!(body["data"]["name"].as_str().unwrap(), team_request["name"].as_str().unwrap());
    assert_eq!(body["data"]["color"].as_str().unwrap(), team_request["color"].as_str().unwrap());
}

#[tokio::test]
async fn admin_get_team_by_id_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Create a team first
    let team_request = json!({
        "name": format!("Team{}", Uuid::new_v4().to_string()[..8].to_string()),
        "color": "#00FF00",
        "formation": "line"
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &token,
        Some(team_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.unwrap();
    let team_id = create_body["data"]["id"].as_str().unwrap();

    // Act
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/teams/{}", test_app.address, team_id),
        &token,
        None,
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["success"].as_bool().unwrap());
    assert_eq!(body["data"]["id"].as_str().unwrap(), team_id);
}

#[tokio::test]
async fn admin_update_team_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Create a team first
    let team_request = json!({
        "name": format!("Team{}", Uuid::new_v4().to_string()[..8].to_string()),
        "color": "#0000FF",
        "formation": "diamond"
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &token,
        Some(team_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.unwrap();
    let team_id = create_body["data"]["id"].as_str().unwrap();

    // Act - Update the team
    let update_request = json!({
        "name": format!("Updated{}", Uuid::new_v4().to_string()[..8].to_string()),
        "color": "#FFFF00",
        "formation": "square"
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::PATCH,
        &format!("{}/admin/teams/{}", test_app.address, team_id),
        &token,
        Some(update_request.clone()),
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["success"].as_bool().unwrap());
    assert_eq!(body["data"]["name"].as_str().unwrap(), update_request["name"].as_str().unwrap());
    assert_eq!(body["data"]["color"].as_str().unwrap(), update_request["color"].as_str().unwrap());
}

#[tokio::test]
async fn admin_delete_team_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Create a team first
    let team_request = json!({
        "name": format!("Del{}", Uuid::new_v4().to_string()[..8].to_string()),
        "color": "#FF00FF",
        "formation": "circle"
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &token,
        Some(team_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.unwrap();
    let team_id = create_body["data"]["id"].as_str().unwrap();

    // Act
    let response = make_authenticated_request(
        &client,
        reqwest::Method::DELETE,
        &format!("{}/admin/teams/{}", test_app.address, team_id),
        &token,
        None,
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["success"].as_bool().unwrap());

    // Verify team is deleted
    let get_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/teams/{}", test_app.address, team_id),
        &token,
        None,
    ).await;

    assert_eq!(404, get_response.status().as_u16());
}

#[tokio::test]
async fn admin_get_team_members_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Create a team first
    let team_request = json!({
        "name": format!("Mem{}", Uuid::new_v4().to_string()[..8].to_string()),
        "color": "#00FFFF",
        "formation": "line"
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &token,
        Some(team_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.unwrap();
    let team_id = create_body["data"]["id"].as_str().unwrap();

    // Act
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/teams/{}/members", test_app.address, team_id),
        &token,
        None,
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["success"].as_bool().unwrap());
    assert!(body["data"].is_array());
    // New team should have no members initially
    assert_eq!(body["data"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn admin_add_team_member_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Create a team
    let team_request = json!({
        "name": format!("Add{}", Uuid::new_v4().to_string()[..8].to_string()),
        "color": "#AABBCC",
        "formation": "diamond"
    });

    let team_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &token,
        Some(team_request),
    ).await;

    let team_body: serde_json::Value = team_response.json().await.unwrap();
    let team_id = team_body["data"]["id"].as_str().unwrap();

    // Get a user to add to the team
    let users_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users?limit=1", test_app.address),
        &token,
        None,
    ).await;

    let users_body: serde_json::Value = users_response.json().await.unwrap();
    let user_id = users_body["data"][0]["id"].as_str().unwrap();

    // Act
    let member_request = json!({
        "user_id": user_id,
        "role": "member"
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams/{}/members", test_app.address, team_id),
        &token,
        Some(member_request),
    ).await;

    // Assert
    assert_eq!(201, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["success"].as_bool().unwrap());
    assert!(body["data"]["id"].is_string());
    assert_eq!(body["data"]["user_id"].as_str().unwrap(), user_id);
    assert_eq!(body["data"]["team_id"].as_str().unwrap(), team_id);
    assert_eq!(body["data"]["role"].as_str().unwrap(), "member");
}

#[tokio::test]
async fn admin_get_leagues_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Act
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/leagues", test_app.address),
        &token,
        None,
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["success"].as_bool().unwrap());
    assert!(body["data"].is_array());
}

#[tokio::test]
async fn admin_create_league_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Act
    let league_request = json!({
        "name": format!("Test League {}", Uuid::new_v4()),
        "description": "A test league for integration testing",
        "max_teams": 16,
        "season_start_date": "2024-01-01T00:00:00Z",
        "season_end_date": "2024-12-31T23:59:59Z"
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", test_app.address),
        &token,
        Some(league_request.clone()),
    ).await;

    // Assert
    assert_eq!(201, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["success"].as_bool().unwrap());
    assert!(body["data"]["id"].is_string());
    assert_eq!(body["data"]["name"].as_str().unwrap(), league_request["name"].as_str().unwrap());
    assert_eq!(body["data"]["max_teams"].as_i64().unwrap(), 16);
}

#[tokio::test]
async fn admin_create_league_with_invalid_dates_fails() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Act - Create league with end date before start date
    let league_request = json!({
        "name": format!("Invalid League {}", Uuid::new_v4()),
        "description": "A league with invalid dates",
        "max_teams": 8,
        "season_start_date": "2024-12-31T23:59:59Z",
        "season_end_date": "2024-01-01T00:00:00Z"
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", test_app.address),
        &token,
        Some(league_request),
    ).await;

    // Assert
    assert_eq!(400, response.status().as_u16());
}

#[tokio::test]
async fn admin_get_league_by_id_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Create a league first
    let league_request = json!({
        "name": format!("League for ID Test {}", Uuid::new_v4()),
        "description": "Test league",
        "max_teams": 12,
        "season_start_date": "2024-03-01T00:00:00Z",
        "season_end_date": "2024-11-30T23:59:59Z"
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

    // Act
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/leagues/{}", test_app.address, league_id),
        &token,
        None,
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["success"].as_bool().unwrap());
    assert_eq!(body["data"]["id"].as_str().unwrap(), league_id);
}

#[tokio::test]
async fn admin_update_league_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Create a league first
    let league_request = json!({
        "name": format!("Original League {}", Uuid::new_v4()),
        "description": "Original description",
        "max_teams": 8,
        "season_start_date": "2024-04-01T00:00:00Z",
        "season_end_date": "2024-10-31T23:59:59Z"
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

    // Act - Update the league
    let update_request = json!({
        "name": format!("Updated League {}", Uuid::new_v4()),
        "is_active": false
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::PATCH,
        &format!("{}/admin/leagues/{}", test_app.address, league_id),
        &token,
        Some(update_request.clone()),
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["success"].as_bool().unwrap());
    assert_eq!(body["data"]["name"].as_str().unwrap(), update_request["name"].as_str().unwrap());
}

#[tokio::test]
async fn admin_routes_with_invalid_token_return_401() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let invalid_token = "invalid.jwt.token";

    // Act & Assert
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users", test_app.address),
        invalid_token,
        None,
    ).await;

    assert_eq!(401, response.status().as_u16());
}

#[tokio::test]
async fn admin_routes_return_proper_error_formats() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Act - Try to get non-existent team
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/teams/{}", test_app.address, Uuid::new_v4()),
        &token,
        None,
    ).await;

    // Assert
    assert_eq!(404, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["error"].is_string());
}