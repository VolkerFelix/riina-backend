use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

mod common;
use common::utils::spawn_app;

#[tokio::test]
async fn test_upcoming_games_endpoint_unauthorized() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Test without authentication token
    let response = client
        .get(&format!("{}/league/games/upcoming", &test_app.address))
        .send()
        .await
        .expect("Failed to send request without auth");

    assert_eq!(response.status(), 401, "Request without auth should return 401");
}

#[tokio::test]
async fn test_upcoming_games_endpoint_with_auth() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create a user and get token
    let username = format!("testuser_{}", Uuid::new_v4());
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
        .expect("Failed to register user");

    assert!(response.status().is_success(), "User registration should succeed");

    // Login to get JWT token
    let login_request = json!({
        "username": username,
        "password": password
    });

    let login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&login_request)
        .send()
        .await
        .expect("Failed to login");

    assert!(login_response.status().is_success(), "Login should succeed");
    
    let login_json = login_response.json::<serde_json::Value>().await
        .expect("Failed to parse login response");
    let token = login_json["token"].as_str().expect("Token not found");

    // Test upcoming games endpoint with authentication
    let response = client
        .get(&format!("{}/league/games/upcoming", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get upcoming games");

    assert!(response.status().is_success(), "Upcoming games request should succeed");

    let games_json = response.json::<serde_json::Value>().await
        .expect("Failed to parse upcoming games response");

    // Validate response structure
    assert_eq!(games_json["success"], true, "Response should indicate success");
    assert!(games_json["data"].is_array(), "Data should be an array");
    assert!(games_json["total_count"].is_number(), "total_count should be a number");

    println!("✅ Upcoming games endpoint working correctly");
}

#[tokio::test]
async fn test_upcoming_games_with_query_params() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create a user and get token
    let username = format!("testuser_{}", Uuid::new_v4());
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
        .expect("Failed to register user");

    let login_request = json!({
        "username": username,
        "password": password
    });

    let login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&login_request)
        .send()
        .await
        .expect("Failed to login");

    let login_json = login_response.json::<serde_json::Value>().await
        .expect("Failed to parse login response");
    let token = login_json["token"].as_str().expect("Token not found");

    // Test with limit parameter
    let response = client
        .get(&format!("{}/league/games/upcoming?limit=5", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get upcoming games with limit");

    assert!(response.status().is_success(), "Request with limit should succeed");

    let games_json = response.json::<serde_json::Value>().await
        .expect("Failed to parse response");

    assert_eq!(games_json["success"], true);
    assert!(games_json["data"].is_array());

    println!("✅ Upcoming games endpoint with query params working correctly");
}