use reqwest::Client;
use uuid::Uuid;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request};

use crate::common::admin_helpers::create_admin_user_and_login;

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
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Act - Try to get non-existent team
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/teams/{}", test_app.address, Uuid::new_v4()),
        &admin.token,
        None,
    ).await;

    // Assert
    assert_eq!(404, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["error"].is_string());
}