// Helper functions for social feature tests
use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use chrono::Utc;
use crate::common::utils::{create_test_user_and_login, UserRegLoginResponse};
use crate::common::workout_data_helpers::{WorkoutData, WorkoutType, upload_workout_data_for_user};

/// Helper function to create user with health profile and upload a workout
pub async fn create_user_with_workout(app_address: &str) -> (UserRegLoginResponse, Uuid) {
    let client = Client::new();
    let user = create_test_user_and_login(app_address).await;

    // Create health profile for stats calculation
    let health_profile_data = json!({
        "age": 25,
        "gender": "male",
        "resting_heart_rate": 60
    });

    let profile_response = client
        .put(&format!("{}/profile/health_profile", app_address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&health_profile_data)
        .send()
        .await
        .expect("Failed to create health profile");

    assert!(profile_response.status().is_success(), "Health profile creation should succeed");

    // Upload a workout
    let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 30);
    let workout_response = upload_workout_data_for_user(&client, app_address, &user.token, &mut workout_data).await;
    assert!(workout_response.is_ok(), "Workout upload should succeed");

    let workout_response_data = workout_response.unwrap();
    let workout_id = Uuid::parse_str(workout_response_data["data"]["sync_id"].as_str().unwrap()).unwrap();
    (user, workout_id)
}
