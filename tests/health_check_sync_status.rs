use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};

#[tokio::test]
async fn test_check_workout_sync_status() {
    let app = spawn_app().await;
    let client = Client::new();

    // Create test user and login
    let user = create_test_user_and_login(&app.address).await;

    // Create a workout with a specific UUID
    let workout_uuid = &Uuid::new_v4().to_string()[..8];
    let workout_data = json!({
        "device_id": "test-device",
        "timestamp": "2024-01-01T10:30:00Z",
        "workout_uuid": workout_uuid,
        "workout_start": "2024-01-01T10:00:00Z",
        "workout_end": "2024-01-01T11:00:00Z",
        "heart_rate_data": [
            {"timestamp": "2024-01-01T10:00:00Z", "heart_rate": 120}
        ],
        "calories_burned": 300
    });

    // Upload the workout
    let response = client
        .post(&format!("{}/health/upload_health", &app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&workout_data)
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success());

    // Check sync status for multiple UUIDs
    let check_request = json!({
        "workout_uuids": [
            workout_uuid,
            "non-existent-uuid-1",
            "non-existent-uuid-2"
        ]
    });

    let response = client
        .post(&format!("{}/health/check_sync_status", &app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&check_request)
        .send()
        .await
        .expect("Failed to execute request");

    let response_status = response.status();
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    if !response_status.is_success() {
        panic!("Request failed with status {}: {}", response_status, body);
    } else {
        let data = &body["data"];

        assert_eq!(data["synced_workouts"].as_array().unwrap().len(), 1);
        assert_eq!(data["unsynced_workouts"].as_array().unwrap().len(), 2);
        assert!(data["synced_workouts"][0].as_str().unwrap() == workout_uuid);
    }
}

#[tokio::test]
async fn test_check_sync_status_empty_list() {
    let app = spawn_app().await;
    let client = Client::new();

    // Create test user and login
    let user = create_test_user_and_login(&app.address).await;

    // Check sync status with empty list
    let check_request = json!({
        "workout_uuids": []
    });

    let response = client
        .post(&format!("{}/health/check_sync_status", &app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&check_request)
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    let data = &body["data"];

    assert_eq!(data["synced_workouts"].as_array().unwrap().len(), 0);
    assert_eq!(data["unsynced_workouts"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_check_sync_status_unauthorized() {
    let app = spawn_app().await;
    let client = Client::new();

    let check_request = json!({
        "workout_uuids": ["some-uuid"]
    });

    let response = client
        .post(&format!("{}/health/check_sync_status", &app.address))
        .json(&check_request)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 401);
}