use reqwest::Client;
use serde_json::json;
use jsonwebtoken::{encode, Header, EncodingKey};
use chrono::{Utc, Duration};
use uuid::Uuid;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};
use riina_backend::models::user::{UserRole, UserStatus};
use riina_backend::middleware::auth::Claims;

// Helper function to create an expired JWT token for testing
fn create_expired_token(user_id: &str, username: &str, secret: &str) -> String {
    let now = Utc::now();
    let expired_time = now.checked_sub_signed(Duration::hours(2)) // Token expired 2 hours ago
        .expect("Valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        role: UserRole::User,
        status: UserStatus::Active,
        exp: expired_time,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ).expect("Failed to create expired token")
}

// Helper function to create a very old expired token (more than 30 days)
fn create_very_old_expired_token(user_id: &str, username: &str, secret: &str) -> String {
    let now = Utc::now();
    let very_old_time = now.checked_sub_signed(Duration::days(35)) // Token expired 35 days ago
        .expect("Valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        role: UserRole::User,
        status: UserStatus::Active,
        exp: very_old_time,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ).expect("Failed to create very old expired token")
}

#[tokio::test]
async fn biometric_refresh_returns_200_for_valid_expired_token() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create a test user and get their token
    let user = create_test_user_and_login(&test_app.address).await;
    let user_id = user.user_id.to_string();
    let username = user.username;

    // Create an expired token for this user
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET environment variable must be set");
    let expired_token = create_expired_token(&user_id, &username, &secret);

    // Act - Try to refresh the expired token
    let refresh_request = json!({
        "token": expired_token
    });

    let response = client
        .post(&format!("{}/biometric-refresh", &test_app.address))
        .json(&refresh_request)
        .send()
        .await
        .expect("Failed to execute biometric refresh request.");

    // Assert
    assert_eq!(200, response.status().as_u16(), "Biometric refresh should succeed for valid expired token");
    
    // Check that the response contains a new token
    let response_body = response.json::<serde_json::Value>().await
        .expect("Failed to parse refresh response as JSON");
    assert!(response_body.get("token").is_some(), "Response should contain a new token");
    
    // Verify the new token is different from the expired one
    let new_token = response_body["token"].as_str().expect("No token in response");
    assert_ne!(expired_token, new_token, "New token should be different from expired token");
}

#[tokio::test]
async fn biometric_refresh_returns_401_for_invalid_token() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Act - Try to refresh with an invalid token
    let refresh_request = json!({
        "token": "invalid_token_here"
    });

    let response = client
        .post(&format!("{}/biometric-refresh", &test_app.address))
        .json(&refresh_request)
        .send()
        .await
        .expect("Failed to execute biometric refresh request.");

    // Assert
    assert_eq!(401, response.status().as_u16(), "Should return 401 for invalid token");
    
    let response_body = response.json::<serde_json::Value>().await
        .expect("Failed to parse error response as JSON");
    assert!(response_body.get("error").is_some(), "Response should contain error message");
}

#[tokio::test]
async fn biometric_refresh_returns_401_for_very_old_token() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create a test user
    let user = create_test_user_and_login(&test_app.address).await;
    let user_id = user.user_id.to_string();
    let username = user.username;

    // Create a very old expired token (more than 30 days)
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET environment variable must be set");
    let very_old_token = create_very_old_expired_token(&user_id, &username, &secret);

    // Act - Try to refresh the very old token
    let refresh_request = json!({
        "token": very_old_token
    });

    let response = client
        .post(&format!("{}/biometric-refresh", &test_app.address))
        .json(&refresh_request)
        .send()
        .await
        .expect("Failed to execute biometric refresh request.");

    // Assert
    assert_eq!(401, response.status().as_u16(), "Should return 401 for very old token");
    
    let response_body = response.json::<serde_json::Value>().await
        .expect("Failed to parse error response as JSON");
    assert!(response_body.get("error").is_some(), "Response should contain error message");
    assert_eq!("Token too old for refresh", response_body["error"], "Should indicate token is too old");
}

#[tokio::test]
async fn biometric_refresh_returns_401_for_nonexistent_user() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create an expired token for a non-existent user
    let non_existent_user_id = Uuid::new_v4().to_string();
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET environment variable must be set");
    let expired_token = create_expired_token(&non_existent_user_id, "nonexistent_user", &secret);

    // Act - Try to refresh the token for non-existent user
    let refresh_request = json!({
        "token": expired_token
    });

    let response = client
        .post(&format!("{}/biometric-refresh", &test_app.address))
        .json(&refresh_request)
        .send()
        .await
        .expect("Failed to execute biometric refresh request.");

    // Assert
    assert_eq!(401, response.status().as_u16(), "Should return 401 for non-existent user");
    
    let response_body = response.json::<serde_json::Value>().await
        .expect("Failed to parse error response as JSON");
    assert!(response_body.get("error").is_some(), "Response should contain error message");
}

#[tokio::test]
async fn biometric_refresh_returns_401_for_malformed_token() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Act - Try to refresh with malformed tokens
    let malformed_tokens = vec![
        "not.a.token",
        "too.few.parts",
        "too.many.parts.here.extra",
        "",
        "invalid_base64_encoding",
    ];

    for malformed_token in malformed_tokens {
        let refresh_request = json!({
            "token": malformed_token
        });

        let response = client
            .post(&format!("{}/biometric-refresh", &test_app.address))
            .json(&refresh_request)
            .send()
            .await
            .expect("Failed to execute biometric refresh request.");

        // Assert
        assert_eq!(401, response.status().as_u16(), 
            "Should return 401 for malformed token: {}", malformed_token);
    }
}

#[tokio::test]
async fn biometric_refresh_returns_401_for_token_with_wrong_signature() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create a test user
    let user = create_test_user_and_login(&test_app.address).await;
    let user_id = user.user_id.to_string();
    let username = user.username;

    // Create an expired token with wrong secret
    let wrong_secret = "wrong_secret_key";
    let expired_token = create_expired_token(&user_id, &username, wrong_secret);

    // Act - Try to refresh the token with wrong signature
    let refresh_request = json!({
        "token": expired_token
    });

    let response = client
        .post(&format!("{}/biometric-refresh", &test_app.address))
        .json(&refresh_request)
        .send()
        .await
        .expect("Failed to execute biometric refresh request.");

    // Assert
    assert_eq!(401, response.status().as_u16(), "Should return 401 for token with wrong signature");
    
    let response_body = response.json::<serde_json::Value>().await
        .expect("Failed to parse error response as JSON");
    assert!(response_body.get("error").is_some(), "Response should contain error message");
}

#[tokio::test]
async fn biometric_refresh_returns_400_for_missing_token() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Act - Try to refresh without token field
    let refresh_request = json!({});

    let response = client
        .post(&format!("{}/biometric-refresh", &test_app.address))
        .json(&refresh_request)
        .send()
        .await
        .expect("Failed to execute biometric refresh request.");

    // Assert
    assert_eq!(400, response.status().as_u16(), "Should return 400 for missing token field");
}

#[tokio::test]
async fn biometric_refresh_returns_400_for_invalid_json() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Act - Try to refresh with invalid JSON
    let response = client
        .post(&format!("{}/biometric-refresh", &test_app.address))
        .header("Content-Type", "application/json")
        .body("invalid json")
        .send()
        .await
        .expect("Failed to execute biometric refresh request.");

    // Assert
    assert_eq!(400, response.status().as_u16(), "Should return 400 for invalid JSON");
}
