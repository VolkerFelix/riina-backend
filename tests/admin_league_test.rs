use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

mod common;
use common::utils::spawn_app;
use common::admin_helpers::{create_test_user_and_login, make_authenticated_request, create_teams_for_test};

#[tokio::test]
async fn admin_get_leagues_returns_list_of_leagues() {
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
async fn admin_create_league_succeeds_with_valid_data() {
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
async fn admin_create_league_with_invalid_max_teams_fails() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Act - Create league with invalid max_teams (0 or negative)
    let league_request = json!({
        "name": format!("Invalid League {}", Uuid::new_v4()),
        "description": "A league with invalid max teams",
        "max_teams": 0
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
async fn admin_get_league_by_id_returns_league_details() {
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
async fn admin_update_league_modifies_league_data() {
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
        "name": format!("Updated League {}", Uuid::new_v4())
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
async fn admin_assign_teams_to_league_works() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Create a league
    let league_request = json!({
        "name": format!("Team Assignment League {}", Uuid::new_v4()),
        "description": "Testing team assignment",
        "max_teams": 4
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
    let league_id = league_body["data"]["id"].as_str().expect("League ID not found");

    // Create teams
    let team_ids = create_teams_for_test(&test_app.address, &token, 2).await;

    // Act - Assign teams to league
    for team_id in &team_ids {
        let assign_request = json!({
            "team_id": team_id
        });

        let response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/leagues/{}/teams", test_app.address, league_id),
            &token,
            Some(assign_request),
        ).await;

        // Assert
        assert_eq!(201, response.status().as_u16());
    }
}