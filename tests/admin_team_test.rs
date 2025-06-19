use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

mod common;
use common::utils::spawn_app;
use common::admin_helpers::{create_test_user_and_login_with_id, make_authenticated_request};

#[tokio::test]
async fn admin_create_team_succeeds_with_valid_data() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let (token, owner_id) = create_test_user_and_login_with_id(&test_app.address).await;

    // Act
    let team_request = json!({
        "name": format!("Test Dragons {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#FF0000",
        "owner_id": owner_id
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &token,
        Some(team_request),
    ).await;

    // Assert
    assert_eq!(201, response.status().as_u16());
    
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["data"]["name"].as_str().unwrap().starts_with("Test Dragons"));
    assert_eq!("#FF0000", body["data"]["color"].as_str().unwrap());
}

#[tokio::test]
async fn admin_get_teams_returns_paginated_results() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let (token, owner_id) = create_test_user_and_login_with_id(&test_app.address).await;

    // Create a test team first
    let team_request = json!({
        "name": format!("Test Team {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#00FF00",
        "owner_id": owner_id
    });

    make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &token,
        Some(team_request),
    ).await;

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
}

#[tokio::test]
async fn admin_get_team_by_id_returns_team_details() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let (token, owner_id) = create_test_user_and_login_with_id(&test_app.address).await;

    // Create a test team first
    let team_request = json!({
        "name": format!("Phoenix Warriors {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#FFA500",
        "owner_id": owner_id
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &token,
        Some(team_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.expect("Failed to parse create response");
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
    assert_eq!(team_id, body["data"]["id"].as_str().unwrap());
    assert!(body["data"]["name"].as_str().unwrap().starts_with("Phoenix Warriors"));
}

#[tokio::test]
async fn admin_update_team_modifies_team_data() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let (token, owner_id) = create_test_user_and_login_with_id(&test_app.address).await;

    // Create a test team first
    let team_request = json!({
        "name": format!("Original Team {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#0000FF",
        "owner_id": owner_id
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &token,
        Some(team_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.expect("Failed to parse create response");
    let team_id = create_body["data"]["id"].as_str().unwrap();

    // Act
    let update_request = json!({
        "name": format!("Updated Team {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#FF00FF",
        "owner_id": owner_id
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::PATCH,
        &format!("{}/admin/teams/{}", test_app.address, team_id),
        &token,
        Some(update_request),
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());
    
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["data"]["name"].as_str().unwrap().starts_with("Updated Team"));
    assert_eq!("#FF00FF", body["data"]["color"].as_str().unwrap());
}

#[tokio::test]
async fn admin_delete_team_removes_team() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let (token, owner_id) = create_test_user_and_login_with_id(&test_app.address).await;

    // Create a test team first
    let team_request = json!({
        "name": format!("Doomed Team {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#666666",
        "owner_id": owner_id
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &token,
        Some(team_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.expect("Failed to parse create response");
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

    // Verify team is deleted by trying to get it
    let get_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/teams/{}", test_app.address, team_id),
        &token,
        None,
    ).await;

    assert_eq!(404, get_response.status().as_u16());
}