use reqwest::Client;
use serde_json::json;

mod common;
use common::utils::{spawn_app, generate_valid_username_suffix};

#[tokio::test]
async fn login_returns_200_for_valid_credentials() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Register a new user first
    let username = format!("protecteduser{}", generate_valid_username_suffix());
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

#[tokio::test]
async fn reset_password_returns_200_for_valid_user() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Register a new user first
    let username = format!("resetuser{}", generate_valid_username_suffix());
    let old_password = "oldpassword123";
    let new_password = "newpassword456";
    let email = format!("{}@example.com", username);

    let user_request = json!({
        "username": username,
        "password": old_password,
        "email": email
    });

    let register_response = client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to execute registration request.");

    assert_eq!(200, register_response.status().as_u16(), "Registration should succeed");

    // Act - Reset password
    let reset_request = json!({
        "username": username,
        "new_password": new_password
    });

    let reset_response = client
        .post(&format!("{}/reset-password", &test_app.address))
        .json(&reset_request)
        .send()
        .await
        .expect("Failed to execute reset password request.");

    // Assert
    assert_eq!(200, reset_response.status().as_u16(), "Password reset should succeed");

    let response_body = reset_response.json::<serde_json::Value>().await
        .expect("Failed to parse reset password response as JSON");
    assert_eq!(response_body.get("success").and_then(|v| v.as_bool()), Some(true));

    // Verify old password no longer works
    let old_login_request = json!({
        "username": username,
        "password": old_password
    });

    let old_login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&old_login_request)
        .send()
        .await
        .expect("Failed to execute login request with old password.");

    assert_eq!(401, old_login_response.status().as_u16(), "Login with old password should fail");

    // Verify new password works
    let new_login_request = json!({
        "username": username,
        "password": new_password
    });

    let new_login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&new_login_request)
        .send()
        .await
        .expect("Failed to execute login request with new password.");

    assert_eq!(200, new_login_response.status().as_u16(), "Login with new password should succeed");

    let login_body = new_login_response.json::<serde_json::Value>().await
        .expect("Failed to parse login response as JSON");
    assert!(login_body.get("token").is_some(), "Response should contain a token");
}

#[tokio::test]
async fn reset_password_returns_404_for_nonexistent_user() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Act - Try to reset password for non-existent user
    let reset_request = json!({
        "username": "nonexistentuser12345",
        "new_password": "newpassword123"
    });

    let response = client
        .post(&format!("{}/reset-password", &test_app.address))
        .json(&reset_request)
        .send()
        .await
        .expect("Failed to execute reset password request.");

    // Assert
    assert_eq!(404, response.status().as_u16(), "Password reset should return 404 for non-existent user");

    let response_body = response.json::<serde_json::Value>().await
        .expect("Failed to parse response as JSON");
    assert!(response_body.get("error").is_some(), "Response should contain an error message");
}

#[tokio::test]
async fn reset_password_accepts_any_valid_password() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Register a new user
    let username = format!("anypassuser{}", generate_valid_username_suffix());
    let old_password = "oldpassword";
    let email = format!("{}@example.com", username);

    let user_request = json!({
        "username": username,
        "password": old_password,
        "email": email
    });

    let register_response = client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to execute registration request.");

    assert_eq!(200, register_response.status().as_u16());

    // Test various password formats (backend doesn't validate complexity)
    let test_passwords = vec!["a", "123", "short", "verylongpasswordwithlotsofcharacters"];

    for new_password in test_passwords {
        // Act - Reset password
        let reset_request = json!({
            "username": username,
            "new_password": new_password
        });

        let reset_response = client
            .post(&format!("{}/reset-password", &test_app.address))
            .json(&reset_request)
            .send()
            .await
            .expect("Failed to execute reset password request.");

        // Assert
        assert_eq!(200, reset_response.status().as_u16(), "Password reset should succeed for password: {}", new_password);

        // Verify new password works
        let login_request = json!({
            "username": username,
            "password": new_password
        });

        let login_response = client
            .post(&format!("{}/login", &test_app.address))
            .json(&login_request)
            .send()
            .await
            .expect("Failed to execute login request.");

        assert_eq!(200, login_response.status().as_u16(), "Login should succeed with new password: {}", new_password);
    }
}