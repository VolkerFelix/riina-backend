use reqwest::Client;
use serde_json::json;

mod common;
use common::utils::spawn_app;

#[tokio::test]
async fn register_user_working() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Register a new user first
    // Use only alphanumeric part of UUID (remove hyphens for valid username)
    let uuid_str = uuid::Uuid::new_v4().to_string().replace("-", "");
    let username = format!("user{}", &uuid_str[..8]); // Use first 8 chars
    let password = "password123";
    let email = format!("{}@example.com", username);

    let user_request = json!({
        "username": username,
        "password": password,
        "email": email
    });

    let response = client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());

    let saved = sqlx::query!("SELECT username, email FROM users WHERE username = $1",
        username
    )
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to fetch saved user.");

    assert_eq!(saved.username, username);
    assert_eq!(saved.email, email);
}

#[tokio::test]
async fn register_user_with_special_characters_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Test cases with invalid usernames
    let invalid_usernames = vec![
        ("José", "special character é"),
        ("Tõnu", "special character õ"),
        ("René", "special character é"),
        ("user name", "contains space"),
        ("user@name", "contains @"),
        ("user#123", "contains #"),
        ("user.name", "contains dot"),
        ("用户", "non-ASCII characters"),
        ("_username", "starts with underscore"),
        ("-username", "starts with hyphen"),
        ("username_", "ends with underscore"),
        ("username-", "ends with hyphen"),
        ("us", "too short (less than 3 chars)"),
        ("user__name", "consecutive underscores"),
        ("user--name", "consecutive hyphens"),
    ];

    for (username, reason) in invalid_usernames {
        let user_request = json!({
            "username": username,
            "password": "password123",
            "email": format!("{}@example.com", uuid::Uuid::new_v4())
        });

        let response = client
            .post(&format!("{}/register_user", &test_app.address))
            .json(&user_request)
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!(
            response.status().as_u16(),
            400,
            "Username '{}' should fail validation (reason: {})",
            username,
            reason
        );

        let response_body: serde_json::Value = response
            .json()
            .await
            .expect("Failed to parse response");

        assert!(
            response_body.get("error").is_some(),
            "Response should contain error message for username '{}' (reason: {})",
            username,
            reason
        );
    }
}

#[tokio::test]
async fn register_user_with_valid_usernames_succeeds() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Test cases with valid username patterns (make them unique with UUID suffix)
    let uuid_str = uuid::Uuid::new_v4().to_string().replace("-", "");
    let suffix = &uuid_str[..8];

    let valid_usernames = vec![
        format!("john_doe_{}", suffix),
        format!("jane_smith_{}", suffix),
        format!("user123_{}", suffix),
        format!("Robert_K_{}", suffix),
        format!("test_user_{}", suffix),
        format!("cool_user_{}", suffix),
        format!("User_Name_{}", suffix),
    ];

    for username in valid_usernames {
        let user_request = json!({
            "username": username,
            "password": "password123",
            "email": format!("{}@example.com", uuid::Uuid::new_v4())
        });

        let response = client
            .post(&format!("{}/register_user", &test_app.address))
            .json(&user_request)
            .send()
            .await
            .expect("Failed to execute request.");

        assert!(
            response.status().is_success(),
            "Username '{}' should be valid and registration should succeed",
            username
        );

        // Verify user was actually created
        let saved = sqlx::query!("SELECT username FROM users WHERE username = $1", username)
            .fetch_one(&test_app.db_pool)
            .await
            .expect("Failed to fetch saved user.");

        assert_eq!(saved.username, username);
    }
}

#[tokio::test]
async fn register_duplicate_username_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let uuid_str = uuid::Uuid::new_v4().to_string().replace("-", "");
    let username = format!("user{}", &uuid_str[..8]);
    let email1 = format!("user1_{}@example.com", &uuid_str[..8]);
    let email2 = format!("user2_{}@example.com", &uuid_str[8..16]);

    // Register first user
    let user_request1 = json!({
        "username": username,
        "password": "password123",
        "email": email1
    });

    let response1 = client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user_request1)
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response1.status().is_success(), "First registration should succeed");

    // Try to register second user with same username
    let user_request2 = json!({
        "username": username,
        "password": "password456",
        "email": email2
    });

    let response2 = client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user_request2)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(
        response2.status().as_u16(),
        409,
        "Second registration with same username should return 409 Conflict"
    );

    let response_body: serde_json::Value = response2
        .json()
        .await
        .expect("Failed to parse response");

    assert_eq!(
        response_body.get("error").and_then(|e| e.as_str()),
        Some("Username already exists"),
        "Error message should indicate username conflict"
    );
}

#[tokio::test]
async fn register_duplicate_email_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let uuid_str = uuid::Uuid::new_v4().to_string().replace("-", "");
    let username1 = format!("user1_{}", &uuid_str[..8]);
    let username2 = format!("user2_{}", &uuid_str[8..16]);
    let email = format!("shared_{}@example.com", &uuid_str[16..24]);

    // Register first user
    let user_request1 = json!({
        "username": username1,
        "password": "password123",
        "email": email
    });

    let response1 = client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user_request1)
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response1.status().is_success(), "First registration should succeed");

    // Try to register second user with same email
    let user_request2 = json!({
        "username": username2,
        "password": "password456",
        "email": email
    });

    let response2 = client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user_request2)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(
        response2.status().as_u16(),
        409,
        "Second registration with same email should return 409 Conflict"
    );

    let response_body: serde_json::Value = response2
        .json()
        .await
        .expect("Failed to parse response");

    assert_eq!(
        response_body.get("error").and_then(|e| e.as_str()),
        Some("Email already exists"),
        "Error message should indicate email conflict"
    );
}