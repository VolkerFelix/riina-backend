use reqwest::Client;
use serde_json::json;

mod common;
use common::utils::{spawn_app, generate_valid_username_suffix};

#[tokio::test]
async fn register_push_token_requires_authentication() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Act - Try to register token without auth
    let token_request = json!({
        "token": "ExponentPushToken[test123]",
        "platform": "ios",
        "device_info": {
            "model": "iPhone 14",
            "os_version": "17.0"
        }
    });

    let response = client
        .post(&format!("{}/notifications/register", &test_app.address))
        .json(&token_request)
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert - Should be unauthorized
    assert_eq!(401, response.status().as_u16(), "Should require authentication");
}

#[tokio::test]
async fn register_push_token_succeeds_with_valid_data() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Register and login a user
    let username = format!("pushuser{}", generate_valid_username_suffix());
    let password = "password123";
    let email = format!("{}@example.com", username);

    let user_request = json!({
        "username": username,
        "password": password,
        "email": email
    });

    client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to register user.");

    let login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&json!({
            "username": username,
            "password": password
        }))
        .send()
        .await
        .expect("Failed to login.");

    let login_body: serde_json::Value = login_response.json().await.expect("Failed to parse login response");
    let token = login_body["token"].as_str().expect("Token not found in response");

    // Act - Register push token
    let push_token_request = json!({
        "token": "ExponentPushToken[test123abc]",
        "platform": "ios",
        "device_info": {
            "model": "iPhone 14 Pro",
            "os_version": "17.2"
        }
    });

    let response = client
        .post(&format!("{}/notifications/register", &test_app.address))
        .bearer_auth(token)
        .json(&push_token_request)
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(200, response.status().as_u16(), "Registration should succeed");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(
        response_body["token"].as_str().unwrap(),
        "ExponentPushToken[test123abc]",
        "Token should match"
    );
    assert_eq!(
        response_body["platform"].as_str().unwrap(),
        "ios",
        "Platform should match"
    );
}

#[tokio::test]
async fn register_push_token_rejects_invalid_platform() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Register and login a user
    let username = format!("pushuser{}", generate_valid_username_suffix());
    let password = "password123";
    let email = format!("{}@example.com", username);

    client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&json!({
            "username": username,
            "password": password,
            "email": email
        }))
        .send()
        .await
        .expect("Failed to register user.");

    let login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&json!({
            "username": username,
            "password": password
        }))
        .send()
        .await
        .expect("Failed to login.");

    let login_body: serde_json::Value = login_response.json().await.expect("Failed to parse login response");
    let token = login_body["token"].as_str().expect("Token not found in response");

    // Act - Try to register with invalid platform
    let push_token_request = json!({
        "token": "ExponentPushToken[test123]",
        "platform": "invalid_platform",
        "device_info": null
    });

    let response = client
        .post(&format!("{}/notifications/register", &test_app.address))
        .bearer_auth(token)
        .json(&push_token_request)
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(400, response.status().as_u16(), "Should reject invalid platform");
}

#[tokio::test]
async fn get_user_tokens_returns_registered_tokens() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Register and login a user
    let username = format!("pushuser{}", generate_valid_username_suffix());
    let password = "password123";
    let email = format!("{}@example.com", username);

    client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&json!({
            "username": username,
            "password": password,
            "email": email
        }))
        .send()
        .await
        .expect("Failed to register user.");

    let login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&json!({
            "username": username,
            "password": password
        }))
        .send()
        .await
        .expect("Failed to login.");

    let login_body: serde_json::Value = login_response.json().await.expect("Failed to parse login response");
    let token = login_body["token"].as_str().expect("Token not found in response");

    // Register two push tokens
    let tokens = vec!["ExponentPushToken[token1]", "ExponentPushToken[token2]"];

    for push_token in &tokens {
        client
            .post(&format!("{}/notifications/register", &test_app.address))
            .bearer_auth(token)
            .json(&json!({
                "token": push_token,
                "platform": "ios",
                "device_info": null
            }))
            .send()
            .await
            .expect("Failed to register push token.");
    }

    // Act - Get user tokens
    let response = client
        .get(&format!("{}/notifications/tokens", &test_app.address))
        .bearer_auth(token)
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(200, response.status().as_u16(), "Should return tokens");

    let response_body: Vec<serde_json::Value> = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body.len(), 2, "Should return 2 tokens");
}

#[tokio::test]
async fn unregister_push_token_deactivates_token() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Register and login a user
    let username = format!("pushuser{}", generate_valid_username_suffix());
    let password = "password123";
    let email = format!("{}@example.com", username);

    client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&json!({
            "username": username,
            "password": password,
            "email": email
        }))
        .send()
        .await
        .expect("Failed to register user.");

    let login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&json!({
            "username": username,
            "password": password
        }))
        .send()
        .await
        .expect("Failed to login.");

    let login_body: serde_json::Value = login_response.json().await.expect("Failed to parse login response");
    let token = login_body["token"].as_str().expect("Token not found in response");

    // Register a push token
    let push_token = "ExponentPushToken[test_unregister]";
    client
        .post(&format!("{}/notifications/register", &test_app.address))
        .bearer_auth(token)
        .json(&json!({
            "token": push_token,
            "platform": "android",
            "device_info": null
        }))
        .send()
        .await
        .expect("Failed to register push token.");

    // Act - Unregister the token
    let response = client
        .post(&format!("{}/notifications/unregister", &test_app.address))
        .bearer_auth(token)
        .json(&json!({
            "token": push_token
        }))
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(200, response.status().as_u16(), "Unregister should succeed");

    // Verify token is no longer in active tokens
    let get_response = client
        .get(&format!("{}/notifications/tokens", &test_app.address))
        .bearer_auth(token)
        .send()
        .await
        .expect("Failed to get tokens.");

    let tokens: Vec<serde_json::Value> = get_response.json().await.expect("Failed to parse response");
    assert_eq!(tokens.len(), 0, "Should have no active tokens after unregister");
}

#[tokio::test]
async fn update_existing_token_when_registering_same_token() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Register and login a user
    let username = format!("pushuser{}", generate_valid_username_suffix());
    let password = "password123";
    let email = format!("{}@example.com", username);

    client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&json!({
            "username": username,
            "password": password,
            "email": email
        }))
        .send()
        .await
        .expect("Failed to register user.");

    let login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&json!({
            "username": username,
            "password": password
        }))
        .send()
        .await
        .expect("Failed to login.");

    let login_body: serde_json::Value = login_response.json().await.expect("Failed to parse login response");
    let token = login_body["token"].as_str().expect("Token not found in response");

    // Register a push token
    let push_token = "ExponentPushToken[same_token]";
    client
        .post(&format!("{}/notifications/register", &test_app.address))
        .bearer_auth(token)
        .json(&json!({
            "token": push_token,
            "platform": "ios",
            "device_info": {"version": "1"}
        }))
        .send()
        .await
        .expect("Failed to register first time.");

    // Act - Register same token with updated info
    let response = client
        .post(&format!("{}/notifications/register", &test_app.address))
        .bearer_auth(token)
        .json(&json!({
            "token": push_token,
            "platform": "android",
            "device_info": {"version": "2"}
        }))
        .send()
        .await
        .expect("Failed to register second time.");

    // Assert
    assert_eq!(200, response.status().as_u16(), "Should update existing token");

    // Verify only one token exists with updated platform
    let get_response = client
        .get(&format!("{}/notifications/tokens", &test_app.address))
        .bearer_auth(token)
        .send()
        .await
        .expect("Failed to get tokens.");

    let tokens: Vec<serde_json::Value> = get_response.json().await.expect("Failed to parse response");
    assert_eq!(tokens.len(), 1, "Should have only one token");
    assert_eq!(
        tokens[0]["platform"].as_str().unwrap(),
        "android",
        "Platform should be updated"
    );
}
