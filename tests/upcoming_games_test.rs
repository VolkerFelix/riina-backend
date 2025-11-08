use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};

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
    let user = create_test_user_and_login(&test_app.address).await;

    // Test upcoming games endpoint with authentication
    let response = client
        .get(&format!("{}/league/games/upcoming", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
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
    let user = create_test_user_and_login(&test_app.address).await;

    // Test with limit parameter
    let response = client
        .get(&format!("{}/league/games/upcoming?limit=5", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
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