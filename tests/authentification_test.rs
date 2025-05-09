use reqwest::Client;
use serde_json::json;

mod common;
use common::utils::spawn_app;

#[tokio::test]
async fn login_returns_200_for_valid_credentials() {
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


    // Act - Try to login
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

    // Assert
    assert_eq!(200, login_response.status().as_u16(), "Login should succeed");
    
    // Check that the response contains a token
    let response_body = login_response.json::<serde_json::Value>().await
        .expect("Failed to parse login response as JSON");
    assert!(response_body.get("token").is_some(), "Response should contain a token");
}

#[tokio::test]
async fn login_returns_401_for_invalid_credentials() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Act - Try to login with non-existent user
    let login_request = json!({
        "username": "nonexistentuser",
        "password": "wrongpassword"
    });

    let response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&login_request)
        .send()
        .await
        .expect("Failed to execute login request.");

    // Assert
    assert_eq!(401, response.status().as_u16());
}