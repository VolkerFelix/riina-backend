use reqwest::Client;
use serde_json::json;

mod common;
use common::utils::spawn_app;

#[tokio::test]
async fn register_user_working() {
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