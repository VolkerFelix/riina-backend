// tests/protected_route_test.rs
use reqwest::Client;
use serde_json::json;

mod common;
use common::utils::spawn_app;

#[tokio::test]
async fn protected_route_returns_401_without_token() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Act - Try to access a protected endpoint without a token
    let response = client
        .get(&format!("{}/protected/resource", &test_app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(401, response.status().as_u16());
}

#[tokio::test]
async fn protected_route_returns_200_with_valid_token() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Register a new user first
    let username = format!("protecteduser{}", uuid::Uuid::new_v4());
    let password = "password123";
    let email = format!("{}@example.com", username);

    let user_request = json!({
        "username": username,
        "password": password,
        "email": email
    });

    let register_response = client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to execute registration request.");

    assert_eq!(200, register_response.status().as_u16(), "Registration should succeed");

    // Login to get a token
    let login_request = json!({
        "username": username,
        "password": password
    });

    let login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&login_request)
        .send()
        .await
        .expect("Failed to execute login request.");

    assert_eq!(200, login_response.status().as_u16(), "Login should succeed");

    let login_json = login_response.json::<serde_json::Value>().await
        .expect("Failed to parse login response as JSON");
    let token = login_json["token"].as_str().expect("Token not found in response");

    // Act - Access protected endpoint with token
    let protected_response = client
        .get(&format!("{}/protected/resource", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(200, protected_response.status().as_u16());
}