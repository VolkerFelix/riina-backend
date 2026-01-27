use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request};

/// Helper function to create a user with a specific username
async fn create_user_with_username(app_address: &str, username: &str) {
    let client = Client::new();
    let _registration_response = client
        .post(format!("{}/register_user", app_address))
        .json(&json!({
            "username": username,
            "email": format!("{}@test.com", username),
            "password": "password123"
        }))
        .send()
        .await
        .expect("Failed to register user");
}

#[tokio::test]
async fn test_search_users_by_username() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create test users with different usernames
    let user1 = create_test_user_and_login(&test_app.address).await;
    let user2_username = format!("alice_{}", Uuid::new_v4().to_string()[..8].to_string());
    let user3_username = format!("alicia_{}", Uuid::new_v4().to_string()[..8].to_string());
    let user4_username = format!("bob_{}", Uuid::new_v4().to_string()[..8].to_string());

    create_user_with_username(&test_app.address, &user2_username).await;
    create_user_with_username(&test_app.address, &user3_username).await;
    create_user_with_username(&test_app.address, &user4_username).await;

    println!("✅ Created 4 test users");

    // Test: Search for users with query "ali" should return alice and alicia
    let search_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/users/search?q=ali", &test_app.address),
        &user1.token,
        None::<serde_json::Value>,
    )
    .await;

    assert!(search_response.status().is_success(), "Search request should succeed");

    let search_json = search_response
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse search response");

    assert_eq!(search_json["success"], true, "Response should be successful");
    let users = search_json["data"]
        .as_array()
        .expect("Data should be an array");

    // Should find at least alice and alicia
    assert!(users.len() >= 2, "Should find at least 2 users matching 'ali'");

    let usernames: Vec<String> = users
        .iter()
        .map(|u| u["username"].as_str().unwrap().to_string())
        .collect();

    assert!(
        usernames.iter().any(|u| u.contains("alice")),
        "Should find alice in results"
    );
    assert!(
        usernames.iter().any(|u| u.contains("alicia")),
        "Should find alicia in results"
    );
    assert!(
        !usernames.iter().any(|u| u.contains("bob")),
        "Should not find bob in results"
    );

    println!("✅ Search by username 'ali' works correctly");

    // Verify response structure
    let first_user = &users[0];
    assert!(first_user["user_id"].is_string(), "user_id should be present");
    assert!(first_user["username"].is_string(), "username should be present");
    // profile_picture_url is optional
    assert!(
        first_user.get("profile_picture_url").is_some(),
        "profile_picture_url field should exist"
    );
}

#[tokio::test]
async fn test_search_users_empty_query() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let user = create_test_user_and_login(&test_app.address).await;

    // Test: Search with no query should return recent users
    let search_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/users/search", &test_app.address),
        &user.token,
        None::<serde_json::Value>,
    )
    .await;

    assert!(search_response.status().is_success(), "Search request should succeed");

    let search_json = search_response
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse search response");

    assert_eq!(search_json["success"], true, "Response should be successful");
    let users = search_json["data"]
        .as_array()
        .expect("Data should be an array");

    // Should return at least the current user
    assert!(users.len() >= 1, "Should return at least 1 user");

    println!("✅ Empty query returns recent active users");
}

#[tokio::test]
async fn test_search_users_with_limit() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let user = create_test_user_and_login(&test_app.address).await;

    // Create several test users
    for i in 1..=10 {
        let username = format!("testuser{}_{}", i, Uuid::new_v4().to_string()[..8].to_string());
        create_user_with_username(&test_app.address, &username).await;
    }

    println!("✅ Created 10 test users");

    // Test: Search with limit=5
    let search_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/users/search?q=testuser&limit=5", &test_app.address),
        &user.token,
        None::<serde_json::Value>,
    )
    .await;

    assert!(search_response.status().is_success(), "Search request should succeed");

    let search_json = search_response
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse search response");

    let users = search_json["data"]
        .as_array()
        .expect("Data should be an array");

    // Should respect the limit
    assert!(users.len() <= 5, "Should return at most 5 users");

    println!("✅ Limit parameter works correctly");
}

#[tokio::test]
async fn test_search_users_case_insensitive() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let user = create_test_user_and_login(&test_app.address).await;

    // Create a user with mixed case username
    let mixed_case_username = format!("TestUser_{}", Uuid::new_v4().to_string()[..8].to_string());
    create_user_with_username(&test_app.address, &mixed_case_username).await;

    // Search with lowercase
    let search_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/users/search?q=testuser", &test_app.address),
        &user.token,
        None::<serde_json::Value>,
    )
    .await;

    assert!(search_response.status().is_success(), "Search request should succeed");

    let search_json = search_response
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse search response");

    let users = search_json["data"]
        .as_array()
        .expect("Data should be an array");

    // Should find the user despite case difference
    let found = users.iter().any(|u| {
        u["username"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("testuser")
    });

    assert!(found, "Should find user with case-insensitive search");

    println!("✅ Case-insensitive search works correctly");
}

#[tokio::test]
async fn test_search_users_unauthorized() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Test: Search without authentication should fail
    let search_response = client
        .get(format!("{}/league/users/search?q=test", &test_app.address))
        .send()
        .await
        .expect("Failed to send request");

    assert!(
        !search_response.status().is_success(),
        "Unauthorized request should fail"
    );

    println!("✅ Unauthorized access is properly blocked");
}

#[tokio::test]
async fn test_search_users_no_results() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let user = create_test_user_and_login(&test_app.address).await;

    // Search for a username that doesn't exist
    let search_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!(
            "{}/league/users/search?q=nonexistentuser12345xyz",
            &test_app.address
        ),
        &user.token,
        None::<serde_json::Value>,
    )
    .await;

    assert!(search_response.status().is_success(), "Search request should succeed");

    let search_json = search_response
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse search response");

    assert_eq!(search_json["success"], true, "Response should be successful");
    let users = search_json["data"]
        .as_array()
        .expect("Data should be an array");

    assert_eq!(users.len(), 0, "Should return empty array for no results");

    println!("✅ No results case handled correctly");
}
