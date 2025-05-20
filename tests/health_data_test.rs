use reqwest::Client;
use serde_json::json;
use chrono::Utc;
use uuid::Uuid;

mod common;
use common::utils::spawn_app;

#[tokio::test]
async fn upload_health_data_working() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Register a new user first
    let username = format!("healthuser{}", Uuid::new_v4());
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

    // Login to get JWT token
    let login_request = json!({
        "username": username,
        "password": password
    });

    let response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&login_request)
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    let login_response: serde_json::Value = response.json().await.expect("Failed to parse login response");
    let token = login_response["token"].as_str().expect("No token in response");

    // Prepare health data
    let health_data = json!({
        "device_id": "test-device-123",
        "timestamp": Utc::now(),
        "steps": 5000,
        "heart_rate": 75.5,
        "sleep": {
            "total_sleep_hours": 7.5,
            "in_bed_time": 1678900000,
            "out_bed_time": 1678920000,
            "time_in_bed": 8.0
        },
        "active_energy_burned": 250.5,
        "additional_metrics": {
            "blood_oxygen": 98,
            "skin_temperature": 36.6
        }
    });

    // Upload health data
    let response = client
        .post(&format!("{}/health/upload_health", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&health_data)
        .send()
        .await
        .expect("Failed to execute request.");

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.expect("Failed to read error response");
        panic!("Health data upload failed with status {}: {}", status, error_body);
    }

    assert!(status.is_success());

    // Verify the data was stored correctly
    let saved = sqlx::query!(
        r#"
        SELECT device_id, steps, heart_rate, active_energy_burned 
        FROM health_data 
        WHERE device_id = $1
        "#,
        "test-device-123"
    )
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to fetch saved health data.");

    assert_eq!(saved.device_id, "test-device-123");
    assert_eq!(saved.steps, Some(5000));
    assert_eq!(saved.heart_rate, Some(75.5));
    assert_eq!(saved.active_energy_burned, Some(250.5));
} 