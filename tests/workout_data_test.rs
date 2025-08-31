use reqwest::Client;
use serde_json::json;
use chrono::Utc;
use sqlx::Row;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request};
use common::workout_data_helpers::{WorkoutData, WorkoutType};

async fn create_test_user_with_health_profile(app_address: &str) -> common::utils::UserRegLoginResponse {
    let client = reqwest::Client::new();
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
    
    user
}

#[tokio::test]
async fn upload_workout_data_working() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_with_health_profile(&test_app.address).await;

    let workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 30);
    let workout_data_json = json!(workout_data);

    // Upload health data
    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(workout_data_json),
    ).await;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.expect("Failed to read error response");
        panic!("Health data upload failed with status {}: {}", status, error_body);
    }

    assert!(status.is_success());

    // Verify the data was stored correctly - query by user_id instead of device_id
    let user_id = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM users WHERE username = $1"
    )
    .bind(&test_user.username)
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to fetch user ID");

    let saved = sqlx::query(
        "SELECT device_id, heart_rate_data, calories_burned FROM workout_data WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1"
    )
    .bind(user_id)
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to fetch saved health data.");

    let device_id: String = saved.get("device_id");
    let heart_rate_data: Option<serde_json::Value> = saved.get("heart_rate_data");
    let calories_burned: Option<i32> = saved.get("calories_burned");

    assert!(device_id.starts_with("test-device-"));
    assert!(heart_rate_data.is_some());
    assert_eq!(calories_burned, Some(450));
    
    // Verify the heart rate data structure and content
    if let Some(hr_data) = heart_rate_data {
        assert!(hr_data.is_array());
        let hr_array = hr_data.as_array().unwrap();
        
        // Should have 31 heart rate readings (one per minute for 30 minutes, plus start point)
        assert_eq!(hr_array.len(), 31);
        
        // Verify structure of first reading
        assert!(hr_array[0]["heart_rate"].as_i64().is_some());
        assert!(hr_array[0]["timestamp"].as_str().is_some());
        
        // Verify structure of last reading
        assert!(hr_array[30]["heart_rate"].as_i64().is_some());
        assert!(hr_array[30]["timestamp"].as_str().is_some());
        
        // Verify heart rate values for intense workout (150+ bpm)
        let first_hr = hr_array[0]["heart_rate"].as_i64().unwrap();
        let mid_hr = hr_array[15]["heart_rate"].as_i64().unwrap(); // Middle of workout
        let last_hr = hr_array[30]["heart_rate"].as_i64().unwrap();
        
        // Intense workout has HR around 150+ with variation
        assert!(first_hr >= 150, "Intense workout HR should be >= 150 bpm, got {}", first_hr);
        assert!(mid_hr >= 150, "Intense workout HR should be >= 150 bpm, got {}", mid_hr);
        assert!(last_hr >= 150, "Intense workout HR should be >= 150 bpm, got {}", last_hr);
        
        println!("Heart rate values: start={}, middle={}, end={}", first_hr, mid_hr, last_hr);
    }
}