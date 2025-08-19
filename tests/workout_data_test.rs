use reqwest::Client;
use serde_json::json;
use chrono::{Utc, Duration};
use sqlx::Row;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request};
use uuid::Uuid;

#[tokio::test]
async fn upload_workout_data_working() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;

    // Prepare workout data with multiple heart rate readings simulating a workout
    let base_time = Utc::now();
    let mut heart_rate_readings = Vec::new();
    
    // Generate 10 minutes of heart rate data simulating a workout progression
    for i in 0..600 { // 600 seconds = 10 minutes, one reading per second
        let time_offset = Duration::seconds(i);
        let workout_progress = i as f64 / 600.0; // 0.0 to 1.0
        
        // Simulate workout: resting -> warmup -> high intensity -> cooldown
        let heart_rate = if workout_progress < 0.1 {
            // Resting phase (0-1 min): 65-70 bpm
            (65.0 + 5.0 * workout_progress * 10.0) as i32
        } else if workout_progress < 0.3 {
            // Warmup phase (1-3 min): 70-120 bpm
            (70.0 + 50.0 * (workout_progress - 0.1) / 0.2) as i32
        } else if workout_progress < 0.8 {
            // High intensity phase (3-8 min): 120-160 bpm with variation
            let base_hr = 120.0 + 40.0 * (workout_progress - 0.3) / 0.5;
            (base_hr + 10.0 * (i as f64 * 0.1).sin()) as i32 // Add some variation
        } else {
            // Cooldown phase (8-10 min): 160-80 bpm
            (160.0 - 80.0 * (workout_progress - 0.8) / 0.2) as i32
        };
        
        heart_rate_readings.push(json!({
            "timestamp": base_time + time_offset,
            "heart_rate": heart_rate
        }));
    }

    let workout_data = json!({
        "device_id": "test-device-123",
        "timestamp": base_time,
        "workout_uuid": &Uuid::new_v4().to_string()[..8],
        "heart_rate": heart_rate_readings,
        "sleep": {
            "total_sleep_hours": 7.5,
            "in_bed_time": 1678900000,
            "out_bed_time": 1678920000,
            "time_in_bed": 8.0
        },
        "calories_burned": 450, // Higher calories for a real workout
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
        Some(workout_data),
    ).await;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.expect("Failed to read error response");
        panic!("Health data upload failed with status {}: {}", status, error_body);
    }

    assert!(status.is_success());

    // Verify the data was stored correctly
    let saved = sqlx::query(
        "SELECT device_id, heart_rate_data, calories_burned FROM workout_data WHERE device_id = $1"
    )
    .bind("test-device-123")
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to fetch saved health data.");

    let device_id: String = saved.get("device_id");
    let heart_rate_data: Option<serde_json::Value> = saved.get("heart_rate_data");
    let calories_burned: Option<i32> = saved.get("calories_burned");

    assert_eq!(device_id, "test-device-123");
    assert!(heart_rate_data.is_some());
    assert_eq!(calories_burned, Some(450));
    
    // Verify the heart rate data structure and content
    if let Some(hr_data) = heart_rate_data {
        assert!(hr_data.is_array());
        let hr_array = hr_data.as_array().unwrap();
        
        // Should have 600 heart rate readings (10 minutes of data)
        assert_eq!(hr_array.len(), 600);
        
        // Verify structure of first reading
        assert!(hr_array[0]["heart_rate"].as_i64().is_some());
        assert!(hr_array[0]["timestamp"].as_str().is_some());
        
        // Verify structure of last reading
        assert!(hr_array[599]["heart_rate"].as_i64().is_some());
        assert!(hr_array[599]["timestamp"].as_str().is_some());
        
        // Verify heart rate progression makes sense
        let first_hr = hr_array[0]["heart_rate"].as_i64().unwrap();
        let mid_hr = hr_array[300]["heart_rate"].as_i64().unwrap(); // Middle of workout
        let last_hr = hr_array[599]["heart_rate"].as_i64().unwrap();
        
        // First should be resting (65-70), middle should be high intensity (>120), last should be cooling down
        assert!(first_hr >= 65 && first_hr <= 70, "Resting HR should be 65-70 bpm, got {}", first_hr);
        assert!(mid_hr > 120, "Peak HR should be >120 bpm, got {}", mid_hr);
        assert!(last_hr < first_hr + 50, "Cooldown HR should not be too high, got {}", last_hr);
        
        println!("Heart rate progression: start={:.1}, peak={:.1}, end={:.1}", first_hr, mid_hr, last_hr);
    }
}

#[tokio::test]
async fn duplicate_workout_uuid_prevention() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;

    // Create workout data with a specific UUID
    let workout_uuid = &Uuid::new_v4().to_string()[..8];
    let base_time = Utc::now();
    
    let workout_data = json!({
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
        "calories_burned": 250
    });

    // First upload - should succeed
    let response1 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(workout_data.clone()),
    ).await;

    assert!(response1.status().is_success(), "First upload should succeed");
    
    let response1_body: serde_json::Value = response1.json().await.expect("Failed to parse response");
    assert_eq!(response1_body["success"], true);
    assert!(response1_body["data"]["game_stats"].is_object(), "Should contain game stats");

    // Second upload with same UUID - should be rejected as duplicate
    let response2 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(workout_data.clone()),
    ).await;

    assert!(!response2.status().is_success(), "Duplicate upload should return an error");
    assert_eq!(response2.status(), 409, "Duplicate upload should return 409 Conflict");
    
    let response2_body: serde_json::Value = response2.json().await.expect("Failed to parse duplicate response");
    assert_eq!(response2_body["success"], false);
    assert!(response2_body["error"].as_str().unwrap().contains("duplicate") || 
            response2_body["error"].as_str().unwrap().contains("already exists"));

    // Verify only one record exists in database
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM workout_data WHERE workout_uuid = $1"
    )
    .bind(workout_uuid)
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to count health data records");

    assert_eq!(count, 1, "Should have exactly one record with this workout UUID");

    // Third upload with different UUID - should succeed
    let different_uuid = &Uuid::new_v4().to_string()[..8];
    let mut workout_data_different = workout_data.clone();
    workout_data_different["workout_uuid"] = json!(different_uuid);

    let response3 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(workout_data_different),
    ).await;

    assert!(response3.status().is_success(), "Different UUID upload should succeed");
    
    let response3_body: serde_json::Value = response3.json().await.expect("Failed to parse third response");
    assert_eq!(response3_body["success"], true);
    assert!(response3_body["data"]["game_stats"].is_object(), "Should contain game stats for new workout");

    // Verify now we have two records with different UUIDs
    let total_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM workout_data WHERE workout_uuid IN ($1, $2)"
    )
    .bind(workout_uuid)
    .bind(different_uuid)
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to count total health data records");

    assert_eq!(total_count, 2, "Should have two records with different workout UUIDs");
}

#[tokio::test]
async fn test_duplicate_workout_detection_by_time() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;

    // Create base workout data with start and end times
    let workout_start = Utc::now() - Duration::hours(2); // 2 hours ago
    let workout_end = workout_start + Duration::minutes(30); // 30 minute workout
    
    let workout_data = json!({
        "device_id": "garmin-watch",
        "timestamp": Utc::now(),
        "workout_uuid": Uuid::new_v4().to_string(),
        "workout_start": workout_start,
        "workout_end": workout_end,
        "heart_rate": [{
            "timestamp": workout_start,
            "heart_rate": 120
        }],
        "calories_burned": 250
    });

    // First upload - should succeed
    let response1 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(workout_data.clone()),
    ).await;

    assert!(response1.status().is_success(), "First workout upload should succeed");

    // Second upload with different UUID but same time (within tolerance) - should be accepted but marked as duplicate
    let mut duplicate_workout = workout_data.clone();
    duplicate_workout["workout_uuid"] = json!(Uuid::new_v4().to_string()); // Different UUID
    duplicate_workout["workout_start"] = json!(workout_start + Duration::seconds(10)); // 10 seconds later
    duplicate_workout["workout_end"] = json!(workout_end + Duration::seconds(10)); // 10 seconds later

    let response2 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(duplicate_workout.clone()),
    ).await;

    assert_eq!(response2.status(), 200, "Should accept duplicate workout but mark it");
    
    let response2_body: serde_json::Value = response2.json().await.expect("Failed to parse duplicate response");
    assert_eq!(response2_body["success"], true);
    assert_eq!(response2_body["data"]["is_duplicate"], true, "Should be marked as duplicate");
    assert_eq!(response2_body["data"]["game_stats"]["stat_changes"]["stamina_change"], 0, "Duplicate should have 0 stamina");
    assert_eq!(response2_body["data"]["game_stats"]["stat_changes"]["strength_change"], 0, "Duplicate should have 0 strength");
    assert!(response2_body["message"].as_str().unwrap().contains("duplicate"));

    // Third upload with different UUID and time outside tolerance - should succeed
    let mut different_workout = workout_data.clone();
    different_workout["workout_uuid"] = json!(Uuid::new_v4().to_string());
    different_workout["workout_start"] = json!(workout_start + Duration::minutes(60)); // 1 hour later
    different_workout["workout_end"] = json!(workout_end + Duration::minutes(60)); // 1 hour later

    let response3 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(different_workout),
    ).await;

    assert!(response3.status().is_success(), "Workout with different time should succeed");

    // Verify we have 3 workouts total (all stored)
    let total_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM workout_data WHERE user_id = (SELECT id FROM users WHERE username = $1)"
    )
    .bind(&test_user.username)
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to count workout records");

    assert_eq!(total_count, 3, "Should have all 3 workouts stored");
    
    // Verify only 2 workouts are not duplicates
    let non_duplicate_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM workout_data WHERE user_id = (SELECT id FROM users WHERE username = $1) AND is_duplicate = false"
    )
    .bind(&test_user.username)
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to count non-duplicate records");

    assert_eq!(non_duplicate_count, 2, "Should have exactly 2 non-duplicate workouts");
}

#[tokio::test]
async fn test_duplicate_detection_edge_cases() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;

    let workout_start = Utc::now() - Duration::hours(1);
    let workout_end = workout_start + Duration::minutes(30);

    // Test 1: Workout with start and end times - should be accepted
    let workout1 = json!({
        "device_id": "device1",
        "timestamp": Utc::now(),
        "workout_uuid": Uuid::new_v4().to_string(),
        "workout_start": workout_start,
        "workout_end": workout_end,
        "calories_burned": 200
    });

    let response1 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(workout1),
    ).await;

    assert!(response1.status().is_success(), "First workout should succeed");

    // Workout at exactly 15 seconds later - should be accepted but marked as duplicate
    let workout2 = json!({
        "device_id": "device2",
        "timestamp": Utc::now(),
        "workout_uuid": Uuid::new_v4().to_string(),
        "workout_start": workout_start + Duration::seconds(15), // Exactly at boundary
        "workout_end": workout_end + Duration::seconds(15),
        "calories_burned": 200
    });

    let response2 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(workout2),
    ).await;

    assert_eq!(response2.status(), 200, "Workout at 15-second boundary should be accepted");
    let response2_body: serde_json::Value = response2.json().await.expect("Failed to parse response");
    assert_eq!(response2_body["data"]["is_duplicate"], true, "Should be marked as duplicate");

    // Test 2: Workout at 16 seconds later - should succeed
    let workout3 = json!({
        "device_id": "device3",
        "timestamp": Utc::now(),
        "workout_uuid": Uuid::new_v4().to_string(),
        "workout_start": workout_start + Duration::seconds(16),
        "workout_end": workout_end + Duration::seconds(16),
        "calories_burned": 200
    });

    let response3 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(workout3),
    ).await;

    assert!(response3.status().is_success(), "Workout at 16 seconds should succeed");
    let response3_body: serde_json::Value = response3.json().await.expect("Failed to parse response");
    assert_eq!(response3_body["data"]["is_duplicate"], false, "Should NOT be marked as duplicate");

    // Test 3: Workout without start/end times - should succeed (no time check)
    let workout4 = json!({
        "device_id": "device4",
        "timestamp": Utc::now(),
        "workout_uuid": Uuid::new_v4().to_string(),
        "calories_burned": 200
        // No workout_start or workout_end
    });

    let response4 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(workout4),
    ).await;

    assert!(response4.status().is_success(), "Workout without times should succeed");
}

#[tokio::test]
async fn test_uuid_duplicate_still_rejected() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_user = create_test_user_and_login(&test_app.address).await;

    let workout_uuid = Uuid::new_v4().to_string();
    let workout_data = json!({
        "device_id": "device1",
        "timestamp": Utc::now(),
        "workout_uuid": &workout_uuid,
        "workout_start": Utc::now() - Duration::hours(1),
        "workout_end": Utc::now() - Duration::minutes(30),
        "calories_burned": 200
    });

    // First upload - should succeed
    let response1 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(workout_data.clone()),
    ).await;

    assert!(response1.status().is_success(), "First workout should succeed");

    // Second upload with SAME UUID - should be rejected (this is a true duplicate)
    let response2 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", &test_app.address),
        &test_user.token,
        Some(workout_data.clone()),
    ).await;

    assert_eq!(response2.status(), 409, "Same UUID should still be rejected");
    let response2_body: serde_json::Value = response2.json().await.expect("Failed to parse response");
    assert_eq!(response2_body["success"], false);
    assert!(response2_body["message"].as_str().unwrap().contains("already been uploaded"));
} 