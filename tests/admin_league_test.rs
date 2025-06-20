use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request};
use common::admin_helpers::{create_admin_user_and_login, create_teams_for_test};

#[tokio::test]
async fn admin_get_leagues_returns_list_of_leagues() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Act
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/leagues", test_app.address),
        &admin.token,
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
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Act
    let league_request = json!({
        "name": format!("Test League {}", &Uuid::new_v4().to_string()[..8]),
        "description": "A test league for integration testing",
        "max_teams": 16
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", test_app.address),
        &admin.token,
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
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Act - Create league with invalid max_teams (0 or negative)
    let league_request = json!({
        "name": format!("Invalid League {}", &Uuid::new_v4().to_string()[..8]),
        "description": "A league with invalid max teams",
        "max_teams": 0
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", test_app.address),
        &admin.token,
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
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a league first
    let league_request = json!({
        "name": format!("League for ID Test {}", &Uuid::new_v4().to_string()[..8]),
        "description": "Test league",
        "max_teams": 12
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

    // Act
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/leagues/{}", test_app.address, league_id),
        &admin.token,
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
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a league first
    let league_request = json!({
        "name": format!("Original League {}", &Uuid::new_v4().to_string()[..8]),
        "description": "Original description",
        "max_teams": 8
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

    // Act - Update the league
    let update_request = json!({
        "name": format!("Updated League {}", &Uuid::new_v4().to_string()[..8])
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::PATCH,
        &format!("{}/admin/leagues/{}", test_app.address, league_id),
        &admin.token,
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
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a league
    let league_request = json!({
        "name": format!("Team Assignment League {}", &Uuid::new_v4().to_string()[..8]),
        "description": "Testing team assignment",
        "max_teams": 4
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
    let league_id = league_body["data"]["id"].as_str().expect("League ID not found");

    // Create teams
    let team_ids = create_teams_for_test(&test_app.address, &admin.token, 2).await;

    // Act - Assign teams to league
    for team_id in &team_ids {
        let assign_request = json!({
            "team_id": team_id
        });

        let response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/leagues/{}/teams", test_app.address, league_id),
            &admin.token,
            Some(assign_request),
        ).await;

        // Assert
        assert_eq!(201, response.status().as_u16());
    }
}