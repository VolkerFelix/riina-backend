use reqwest::Client;
use serde_json::json;
use chrono::{Utc, Duration};
use sqlx::Row;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};

use crate::common::utils::make_authenticated_request;

#[tokio::test]
async fn upload_health_data_working() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;

    // Prepare health data with multiple heart rate readings simulating a workout
    let base_time = Utc::now();
    let mut heart_rate_readings = Vec::new();
    
    // Generate 10 minutes of heart rate data simulating a workout progression
    for i in 0..600 { // 600 seconds = 10 minutes, one reading per second
        let time_offset = Duration::seconds(i);
        let workout_progress = i as f64 / 600.0; // 0.0 to 1.0
        
        // Simulate workout: resting -> warmup -> high intensity -> cooldown
        let heart_rate = if workout_progress < 0.1 {
            // Resting phase (0-1 min): 65-70 bpm
            65.0 + 5.0 * workout_progress * 10.0
        } else if workout_progress < 0.3 {
            // Warmup phase (1-3 min): 70-120 bpm
            70.0 + 50.0 * (workout_progress - 0.1) / 0.2
        } else if workout_progress < 0.8 {
            // High intensity phase (3-8 min): 120-160 bpm with variation
            let base_hr = 120.0 + 40.0 * (workout_progress - 0.3) / 0.5;
            base_hr + 10.0 * (i as f64 * 0.1).sin() // Add some variation
        } else {
            // Cooldown phase (8-10 min): 160-80 bpm
            160.0 - 80.0 * (workout_progress - 0.8) / 0.2
        };
        
        heart_rate_readings.push(json!({
            "timestamp": base_time + time_offset,
            "heart_rate": heart_rate
        }));
    }

    let health_data = json!({
        "device_id": "test-device-123",
        "timestamp": base_time,
        "heart_rate": heart_rate_readings,
        "sleep": {
            "total_sleep_hours": 7.5,
            "in_bed_time": 1678900000,
            "out_bed_time": 1678920000,
            "time_in_bed": 8.0
        },
        "active_energy_burned": 450.75, // Higher calories for a real workout
        "additional_metrics": {
            "blood_oxygen": 98,
            "skin_temperature": 36.6
        }
    });

    // Upload health data
    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(health_data),
    ).await;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.expect("Failed to read error response");
        panic!("Health data upload failed with status {}: {}", status, error_body);
    }

    assert!(status.is_success());

    // Verify the data was stored correctly
    let saved = sqlx::query(
        "SELECT device_id, heart_rate_data, active_energy_burned FROM health_data WHERE device_id = $1"
    )
    .bind("test-device-123")
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to fetch saved health data.");

    let device_id: String = saved.get("device_id");
    let heart_rate_data: Option<serde_json::Value> = saved.get("heart_rate_data");
    let active_energy_burned: Option<f32> = saved.get("active_energy_burned");

    assert_eq!(device_id, "test-device-123");
    assert!(heart_rate_data.is_some());
    assert_eq!(active_energy_burned, Some(450.75));
    
    // Verify the heart rate data structure and content
    if let Some(hr_data) = heart_rate_data {
        assert!(hr_data.is_array());
        let hr_array = hr_data.as_array().unwrap();
        
        // Should have 600 heart rate readings (10 minutes of data)
        assert_eq!(hr_array.len(), 600);
        
        // Verify structure of first reading
        assert!(hr_array[0]["heart_rate"].as_f64().is_some());
        assert!(hr_array[0]["timestamp"].as_str().is_some());
        
        // Verify structure of last reading
        assert!(hr_array[599]["heart_rate"].as_f64().is_some());
        assert!(hr_array[599]["timestamp"].as_str().is_some());
        
        // Verify heart rate progression makes sense
        let first_hr = hr_array[0]["heart_rate"].as_f64().unwrap();
        let mid_hr = hr_array[300]["heart_rate"].as_f64().unwrap(); // Middle of workout
        let last_hr = hr_array[599]["heart_rate"].as_f64().unwrap();
        
        // First should be resting (65-70), middle should be high intensity (>120), last should be cooling down
        assert!(first_hr >= 65.0 && first_hr <= 70.0, "Resting HR should be 65-70 bpm, got {}", first_hr);
        assert!(mid_hr > 120.0, "Peak HR should be >120 bpm, got {}", mid_hr);
        assert!(last_hr < first_hr + 50.0, "Cooldown HR should not be too high, got {}", last_hr);
        
        println!("Heart rate progression: start={:.1}, peak={:.1}, end={:.1}", first_hr, mid_hr, last_hr);
    }
}

#[tokio::test]
async fn duplicate_workout_uuid_prevention() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;

    // Create workout data with a specific UUID
    let workout_uuid = "apple-health-workout-12345-abcdef";
    let base_time = Utc::now();
    
    let health_data = json!({
        "device_id": "apple-health-kit",
        "timestamp": base_time,
        "workout_uuid": workout_uuid,
        "heart_rate": [
            {
                "timestamp": base_time,
                "heart_rate": 120
            },
            {
                "timestamp": base_time + Duration::seconds(60),
                "heart_rate": 130
            }
        ],
        "active_energy_burned": 250.0
    });

    // First upload - should succeed
    let response1 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(health_data.clone()),
    ).await;

    assert!(response1.status().is_success(), "First upload should succeed");
    
    let response1_body: serde_json::Value = response1.json().await.expect("Failed to parse response");
    assert_eq!(response1_body["success"], true);
    assert!(response1_body["data"]["game_stats"].is_object(), "Should contain game stats");

    // Second upload with same UUID - should be detected as duplicate
    let response2 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(health_data.clone()),
    ).await;

    assert!(response2.status().is_success(), "Duplicate response should still be 200 OK");
    
    let response2_body: serde_json::Value = response2.json().await.expect("Failed to parse duplicate response");
    assert_eq!(response2_body["success"], true);
    assert_eq!(response2_body["data"]["duplicate"], true);
    assert_eq!(response2_body["data"]["workout_uuid"], workout_uuid);
    assert!(response2_body["message"].as_str().unwrap().contains("already processed"));

    // Verify only one record exists in database
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM health_data WHERE workout_uuid = $1"
    )
    .bind(workout_uuid)
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to count health data records");

    assert_eq!(count, 1, "Should have exactly one record with this workout UUID");

    // Third upload with different UUID - should succeed
    let different_uuid = "apple-health-workout-67890-fedcba";
    let mut health_data_different = health_data.clone();
    health_data_different["workout_uuid"] = json!(different_uuid);

    let response3 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(health_data_different),
    ).await;

    assert!(response3.status().is_success(), "Different UUID upload should succeed");
    
    let response3_body: serde_json::Value = response3.json().await.expect("Failed to parse third response");
    assert_eq!(response3_body["success"], true);
    assert!(response3_body["data"]["duplicate"].is_null() || response3_body["data"]["duplicate"] == false);

    // Verify now we have two records with different UUIDs
    let total_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM health_data WHERE workout_uuid IN ($1, $2)"
    )
    .bind(workout_uuid)
    .bind(different_uuid)
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to count total health data records");

    assert_eq!(total_count, 2, "Should have two records with different workout UUIDs");
} 