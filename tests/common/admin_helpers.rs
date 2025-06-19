use reqwest::{Client, Response};
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;

/// Helper function to create an admin user and get auth token
pub async fn create_test_user_and_login(app_address: &str) -> String {
    let client = Client::new();
    let username = format!("adminuser{}", Uuid::new_v4());
    let password = "password123";
    let email = format!("{}@example.com", username);

    // Register user
    let user_request = json!({
        "username": username,
        "password": password,
        "email": email
    });

    let register_response = client
        .post(&format!("{}/register_user", app_address))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to register user");

    assert_eq!(200, register_response.status().as_u16());

    // Promote user to admin role using direct database access
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");
    
    sqlx::query!(
        "UPDATE users SET role = 'admin' WHERE username = $1",
        username
    )
    .execute(&pool)
    .await
    .expect("Failed to promote user to admin");

    // Login and get token
    let login_request = json!({
        "username": username,
        "password": password
    });

    let login_response = client
        .post(&format!("{}/login", app_address))
        .json(&login_request)
        .send()
        .await
        .expect("Failed to login");

    assert_eq!(200, login_response.status().as_u16());

    let login_body: serde_json::Value = login_response
        .json()
        .await
        .expect("Failed to parse login response");

    login_body["token"].as_str().unwrap().to_string()
}

/// Helper function to create an admin user and get both auth token and user ID
pub async fn create_test_user_and_login_with_id(app_address: &str) -> (String, String) {
    let client = Client::new();
    let username = format!("adminuser{}", Uuid::new_v4());
    let password = "password123";
    let email = format!("{}@example.com", username);

    // Register user
    let user_request = json!({
        "username": username,
        "password": password,
        "email": email
    });

    let register_response = client
        .post(&format!("{}/register_user", app_address))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to register user");

    assert_eq!(200, register_response.status().as_u16());

    // Get user ID from database
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");
    
    let user_record = sqlx::query!(
        "SELECT id FROM users WHERE username = $1",
        username
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to get user ID");
    
    let user_id = user_record.id.to_string();

    // Promote user to admin role
    sqlx::query!(
        "UPDATE users SET role = 'admin' WHERE username = $1",
        username
    )
    .execute(&pool)
    .await
    .expect("Failed to promote user to admin");

    // Login and get token
    let login_request = json!({
        "username": username,
        "password": password
    });

    let login_response = client
        .post(&format!("{}/login", app_address))
        .json(&login_request)
        .send()
        .await
        .expect("Failed to login");

    assert_eq!(200, login_response.status().as_u16());

    let login_body: serde_json::Value = login_response
        .json()
        .await
        .expect("Failed to parse login response");

    let token = login_body["token"].as_str().unwrap().to_string();
    (token, user_id)
}

/// Helper function to make authenticated requests
pub async fn make_authenticated_request(
    client: &Client,
    method: reqwest::Method,
    url: &str,
    token: &str,
    body: Option<serde_json::Value>,
) -> Response {
    let mut request = client.request(method, url)
        .header("Authorization", format!("Bearer {}", token));

    if let Some(json_body) = body {
        request = request.json(&json_body);
    }

    request
        .send()
        .await
        .expect("Failed to execute request")
}

/// Helper function to create teams for testing
pub async fn create_teams_for_test(app_address: &str, token: &str, count: usize) -> Vec<String> {
    let client = Client::new();
    let mut team_ids = Vec::new();

    // Create regular users first to use as team owners
    let mut user_ids = Vec::new();
    for i in 0..count {
        let username = format!("teamowner{}{}", i, Uuid::new_v4());
        let password = "password123";
        let email = format!("{}@example.com", username);

        // Create user
        let user_request = json!({
            "username": username,
            "password": password,
            "email": email
        });

        let user_response = client
            .post(&format!("{}/register_user", app_address))
            .json(&user_request)
            .send()
            .await
            .expect("Failed to register user");

        assert_eq!(200, user_response.status().as_u16());
        
        // Get user ID from database query
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");
        let pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to database");
        
        let user_record = sqlx::query!(
            "SELECT id FROM users WHERE username = $1",
            username
        )
        .fetch_one(&pool)
        .await
        .expect("Failed to get user ID");
        
        user_ids.push(user_record.id.to_string());
    }

    // Now create teams with the user IDs as owners
    for i in 0..count {
        let team_request = json!({
            "name": format!("Test Team {} {}", i + 1, &Uuid::new_v4().to_string()[..8]),
            "color": format!("#{:06X}", (i * 0x111111) % 0xFFFFFF),
            "owner_id": user_ids[i]
        });

        let response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/teams", app_address),
            token,
            Some(team_request),
        ).await;

        assert_eq!(201, response.status().as_u16());
        let body: serde_json::Value = response.json().await.expect("Failed to parse response");
        let team_id = body["data"]["id"].as_str().expect("Team ID not found").to_string();
        team_ids.push(team_id);
    }

    team_ids
}