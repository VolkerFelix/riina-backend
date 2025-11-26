use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use chrono::Utc;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, delete_test_user};
use common::admin_helpers::create_admin_user_and_login;
use common::workout_data_helpers::{WorkoutData, WorkoutIntensity, upload_workout_data_for_user, WorkoutSyncRequest};

#[tokio::test]
async fn test_check_workout_sync_status() {
    let app = spawn_app().await;
    let client = Client::new();

    // Create test user and login
    let user = create_test_user_and_login(&app.address).await;
    
    // Create health profile for stats calculation
    let health_profile_data = json!({
        "age": 25,
        "gender": "male",
        "resting_heart_rate": 60
    });
    
    let profile_response = client
        .put(&format!("{}/profile/health_profile", &app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&health_profile_data)
        .send()
        .await
        .expect("Failed to create health profile");
    
    if !profile_response.status().is_success() {
        let status = profile_response.status();
        let error_body = profile_response.text().await.expect("Failed to read error response");
        panic!("Health profile creation failed with status {}: {}", status, error_body);
    }

    // Create a workout with a specific UUID
    let workout_uuid = &Uuid::new_v4().to_string()[..8];
    let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30);
    workout_data.workout_uuid = workout_uuid.to_string();

    // Upload the workout
    let response = upload_workout_data_for_user(&client, &app.address, &user.token, &mut workout_data).await;
    assert!(response.is_ok(), "Workout upload should succeed");
    let status = response.is_ok();
    if !status {
        let error_body = response.err().unwrap();
        panic!("Workout upload failed with status {}: {}", status, error_body);
    }

    // Check sync status for multiple workouts
    let check_request = json!({
        "workouts": [
            {
                "id": workout_uuid,
                "start": workout_data.workout_start,
                "end": workout_data.workout_end,
                "calories": workout_data.calories_burned
            },
            {
                "id": "non-existent-uuid-1",
                "start": "2024-01-01T12:00:00Z",
                "end": "2024-01-01T13:00:00Z",
                "calories": 100
            },
            {
                "id": "non-existent-uuid-2",
                "start": "2024-01-01T14:00:00Z",
                "end": "2024-01-01T15:00:00Z",
                "calories": 100
            }
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

        // The uploaded workout should NOT be in the unsynced list (it matches by time)
        // Only the two non-existent workouts should be unsynced
        assert_eq!(data["unsynced_workouts"].as_array().unwrap().len(), 2);
        assert!(data["unsynced_workouts"].as_array().unwrap().contains(&json!("non-existent-uuid-1")));
        assert!(data["unsynced_workouts"].as_array().unwrap().contains(&json!("non-existent-uuid-2")));
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
        "workouts": []
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

    assert_eq!(data["unsynced_workouts"].as_array().unwrap().len(), 0);

}

#[tokio::test]
async fn test_check_sync_status_unauthorized() {
    let app = spawn_app().await;
    let client = Client::new();

    let check_request = json!({
        "workouts": [{
            "id": "some-uuid",
            "start": "2024-01-01T10:00:00Z",
            "end": "2024-01-01T11:00:00Z"
        }]
    });

    let response = client
        .post(&format!("{}/health/check_sync_status", &app.address))
        .json(&check_request)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 401);

}