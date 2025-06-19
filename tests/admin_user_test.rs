use reqwest::Client;
use serde_json::json;

mod common;
use common::utils::spawn_app;
use common::admin_helpers::{create_test_user_and_login, create_test_user_and_login_with_id, make_authenticated_request};

#[tokio::test]
async fn admin_get_users_returns_paginated_results() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

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
}

#[tokio::test]
async fn admin_get_users_with_search_filters_results() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

    // Act
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users?search=admin&limit=5", test_app.address),
        &token,
        None,
    ).await;

    // Assert
    assert_eq!(200, response.status().as_u16());
    
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["data"].is_array());
    let pagination = &body["pagination"];
    assert_eq!(5, pagination["limit"].as_i64().unwrap_or(0));
}

#[tokio::test]
async fn admin_get_user_by_id_returns_user_details() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let (token, user_id) = create_test_user_and_login_with_id(&test_app.address).await;

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
    assert_eq!(user_id, body["data"]["id"].as_str().unwrap());
}

#[tokio::test]
async fn admin_update_user_status_changes_user_status() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let (token, user_id) = create_test_user_and_login_with_id(&test_app.address).await;

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
    assert_eq!("inactive", body["data"]["status"].as_str().unwrap());
}

#[tokio::test]
async fn admin_get_users_without_team_returns_filtered_users() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let token = create_test_user_and_login(&test_app.address).await;

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
}