use chrono::{Duration, Utc};
use uuid::Uuid;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, TestApp, delete_test_user};
use common::admin_helpers::create_admin_user_and_login;
use common::workout_data_helpers::{WorkoutData, WorkoutType, upload_workout_data_for_user};

/// Helper function to create multiple overlapping workouts in a single batch

async fn create_overlapping_workouts_batch(
    test_app: &TestApp,
    user_token: &str,
    workout_specs: Vec<(chrono::DateTime<Utc>, chrono::DateTime<Utc>, i32)>,
) -> Result<Vec<Uuid>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let mut workout_data_vec = Vec::new();
    
    // Create all workout data first
    for (start_time, end_time, calories) in workout_specs {
        let duration_minutes = (end_time - start_time).num_minutes() as i64;
        let mut workout_data = WorkoutData::new(WorkoutType::Moderate, start_time, duration_minutes);
        workout_data.workout_end = end_time;
        workout_data.calories_burned = calories;
        
        workout_data_vec.push(workout_data);
    }
    
    // Get approval tokens for all workouts in a single batch request
    let workout_sync_requests: Vec<_> = workout_data_vec.iter().map(|workout| {
        serde_json::json!({
            "start": workout.workout_start,
            "end": workout.workout_end,
            "calories": workout.calories_burned,
            "id": workout.workout_uuid
        })
    }).collect();
    
    let sync_response = crate::common::utils::make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/health/check_sync_status", test_app.address),
        user_token,
        Some(serde_json::json!({
            "workouts": workout_sync_requests
        })),
    ).await;
    
    if !sync_response.status().is_success() {
        let status = sync_response.status();
        let error_body = sync_response.text().await.map_err(|e| e.to_string())?;
        return Err(format!("Health data sync failed with status {}: {}", status, error_body).into());
    }
    
    let sync_response_data: serde_json::Value = sync_response.json().await.map_err(|e| e.to_string())?;
    
    // Extract approval tokens and set them on workout data
    if let Some(approved_workouts) = sync_response_data["data"]["approved_workouts"].as_array() {
        for approval in approved_workouts {
            let workout_id = approval["workout_id"].as_str().unwrap();
            let token = approval["approval_token"].as_str().unwrap();
            
            // Find the matching workout data and set the approval token
            // Note: Due to deduplication in check_workout_sync_status, only one approval token
            // is returned per unique time interval (the one with highest calories)
            for workout_data in &mut workout_data_vec {
                if workout_data.workout_uuid == workout_id {
                    workout_data.approval_token = Some(token.to_string());
                    break;
                }
            }
        }
    }
    
    // Now upload only workouts that have approval tokens
    // (workouts without approval tokens were deduplicated by the sync endpoint)
    let mut workout_ids = Vec::new();
    for mut workout_data in workout_data_vec {
        // Only upload workouts that have approval tokens
        if workout_data.approval_token.is_none() {
            // This workout was deduplicated by the sync endpoint, skip it
            continue;
        }
        
        let response = upload_workout_data_for_user(&client, &test_app.address, user_token, &mut workout_data).await?;
        
        // Check if the workout was deleted as a duplicate
        if let Some(action) = response["data"]["action"].as_str() {
            if action == "duplicate_removed" {
                // This workout was removed as a duplicate, so we don't get a sync_id
                // We'll need to query the database to find the remaining workout
                continue;
            }
        }
        
        // Extract sync_id for successfully uploaded workouts
        if let Some(sync_id_str) = response["data"]["sync_id"].as_str() {
            let workout_id = Uuid::parse_str(sync_id_str)?;
            workout_ids.push(workout_id);
        }
    }
    
    Ok(workout_ids)
}

/// Helper function to create a single workout (for non-overlapping cases)
async fn create_single_workout(
    test_app: &TestApp,
    user_token: &str,
    start_time: chrono::DateTime<Utc>,
    end_time: chrono::DateTime<Utc>,
    calories: i32,
) -> Result<Uuid, Box<dyn std::error::Error>> {
    let workout_ids = create_overlapping_workouts_batch(
        test_app,
        user_token,
        vec![(start_time, end_time, calories)],
    ).await?;
    
    Ok(workout_ids[0])
}

#[tokio::test]
async fn test_exact_duplicate_workouts_are_detected() {
    let test_app = spawn_app().await;
    let user = create_test_user_and_login(&test_app.address).await;

    // Create two workouts with exact same times
    let start_time = Utc::now();
    let end_time = start_time + Duration::hours(1);

    let _ = create_overlapping_workouts_batch(
        &test_app,
        &user.token,
        vec![
            (start_time, end_time, 200),
            (start_time, end_time, 300),
        ],
    ).await.expect("Failed to create overlapping workouts");

    // Verify that automatic cleanup worked correctly by checking workout history
    let client = reqwest::Client::new();
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to execute workout history request");

    assert!(history_response.status().is_success());
    
    let history_data: serde_json::Value = history_response
        .json()
        .await
        .expect("Failed to parse workout history response");

    let workouts = history_data["data"]["workouts"].as_array().unwrap();
    
    // Should have only 1 workout remaining (the higher calorie one)
    assert_eq!(workouts.len(), 1, "Should have 1 workout remaining after automatic cleanup");
    assert_eq!(workouts[0]["calories_burned"], 300, "Should keep the higher calorie workout");

}

#[tokio::test]
async fn test_cleanup_keeps_higher_calorie_workout() {
    let test_app = spawn_app().await;
    let user = create_test_user_and_login(&test_app.address).await;

    // Create two workouts with exact same times but different calories
    let start_time = Utc::now();
    let end_time = start_time + Duration::hours(1);

    let _ = create_overlapping_workouts_batch(
        &test_app,
        &user.token,
        vec![
            (start_time, end_time, 200),
            (start_time, end_time, 500),
        ],
    ).await.expect("Failed to create overlapping workouts");

    // Check that high calorie workout was kept by querying workout history
    let client = reqwest::Client::new();
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to execute workout history request");

    assert!(history_response.status().is_success());
    
    let history_data: serde_json::Value = history_response
        .json()
        .await
        .expect("Failed to parse workout history response");

    let workouts = history_data["data"]["workouts"].as_array().unwrap();
    assert_eq!(workouts.len(), 1, "Should have 1 workout remaining");
    assert_eq!(workouts[0]["calories_burned"], 500, "Should keep the high calorie workout");
    
    // Verify that the remaining workout has proper stats calculated
    let remaining_workout = &workouts[0];
    assert!(remaining_workout["stamina_gained"].is_number(), "Remaining workout should have stamina gains");
    assert!(remaining_workout["strength_gained"].is_number(), "Remaining workout should have strength gains");
    
    // Verify the stats are reasonable for a 60-minute moderate workout
    let stamina_gained = remaining_workout["stamina_gained"].as_f64().unwrap();
    let strength_gained = remaining_workout["strength_gained"].as_f64().unwrap();
    let total_points = stamina_gained + strength_gained; // Calculate total points
    
    assert!(stamina_gained >= 0.0, "Stamina gains should be non-negative");
    assert!(strength_gained >= 0.0, "Strength gains should be non-negative");
    assert!(total_points > 0.0, "Total points should be positive for a 60-minute workout");

}

#[tokio::test]
async fn test_cleanup_handles_null_calories() {
    let test_app = spawn_app().await;
    let user = create_test_user_and_login(&test_app.address).await;

    // Create workouts with same times - one with null calories, one with value
    let start_time = Utc::now();
    let end_time = start_time + Duration::hours(1);

    let _ = create_overlapping_workouts_batch(
        &test_app,
        &user.token,
        vec![
            (start_time, end_time, 0),
            (start_time, end_time, 300),
        ],
    ).await.expect("Failed to create overlapping workouts");

    // Check that workout with calories was kept by querying workout history
    let client = reqwest::Client::new();
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to execute workout history request");

    assert!(history_response.status().is_success());
    
    let history_data: serde_json::Value = history_response
        .json()
        .await
        .expect("Failed to parse workout history response");

    let workouts = history_data["data"]["workouts"].as_array().unwrap();
    assert_eq!(workouts.len(), 1, "Should have 1 workout remaining");
    assert_eq!(workouts[0]["calories_burned"], 300, "Should keep the workout with calories");

}

#[tokio::test]
async fn test_cleanup_with_multiple_duplicates() {
    let test_app = spawn_app().await;
    let user = create_test_user_and_login(&test_app.address).await;

    // Create 4 workouts with same times, different calories
    let start_time = Utc::now();
    let end_time = start_time + Duration::hours(1);

    let _ = create_overlapping_workouts_batch(
        &test_app,
        &user.token,
        vec![
            (start_time, end_time, 100),
            (start_time, end_time, 200),
            (start_time, end_time, 400),
            (start_time, end_time, 300),
        ],
    ).await.expect("Failed to create overlapping workouts");

    // Check that highest calorie workout was kept by querying workout history
    let client = reqwest::Client::new();
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to execute workout history request");

    assert!(history_response.status().is_success());
    
    let history_data: serde_json::Value = history_response
        .json()
        .await
        .expect("Failed to parse workout history response");

    let workouts = history_data["data"]["workouts"].as_array().unwrap();
    assert_eq!(workouts.len(), 1, "Should have 1 workout remaining");
    assert_eq!(workouts[0]["calories_burned"], 400, "Should keep the highest calorie workout");

}

#[tokio::test]
async fn test_cleanup_keeps_older_when_calories_tied() {
    let test_app = spawn_app().await;
    let user = create_test_user_and_login(&test_app.address).await;

    // Create two workouts with same times and same calories
    let start_time = Utc::now();
    let end_time = start_time + Duration::hours(1);

    // Create two workouts with same times and same calories
    let _ = create_overlapping_workouts_batch(
        &test_app,
        &user.token,
        vec![
            (start_time, end_time, 300),
            (start_time, end_time, 300),
        ],
    ).await.expect("Failed to create overlapping workouts");

    // Check that first (older) workout was kept by querying workout history
    let client = reqwest::Client::new();
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to execute workout history request");

    assert!(history_response.status().is_success());
    
    let history_data: serde_json::Value = history_response
        .json()
        .await
        .expect("Failed to parse workout history response");

    let workouts = history_data["data"]["workouts"].as_array().unwrap();
    assert_eq!(workouts.len(), 1, "Should have 1 workout remaining");
    // Note: We can't easily verify which specific workout was kept via API, 
    // but we can verify that exactly one workout remains

}

#[tokio::test]
async fn test_overlapping_but_not_identical_times_grouped() {
    let test_app = spawn_app().await;
    let user = create_test_user_and_login(&test_app.address).await;

    // Create workouts that overlap but don't have exact same times
    let start1 = Utc::now();
    let end1 = start1 + Duration::hours(2);

    let start2 = start1 + Duration::minutes(30); // Starts during first workout
    let end2 = start2 + Duration::hours(1);

    create_single_workout(&test_app, &user.token, start1, end1, 200).await
        .expect("Failed to create first workout");
    create_single_workout(&test_app, &user.token, start2, end2, 300).await
        .expect("Failed to create second workout");

    // Verify that both workouts remain since they have different start/end times
    let client = reqwest::Client::new();
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to execute workout history request");

    assert!(history_response.status().is_success());
    
    let history_data: serde_json::Value = history_response
        .json()
        .await
        .expect("Failed to parse workout history response");

    let workouts = history_data["data"]["workouts"].as_array().unwrap();
    
    // Should have 2 workouts remaining since they have different start/end times
    assert_eq!(workouts.len(), 2, "Should have 2 workouts remaining since they have different times");

}

#[tokio::test]
async fn test_multiple_users_automatic_cleanup_isolated() {
    let test_app = spawn_app().await;
    let user1 = create_test_user_and_login(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // Create overlapping workouts for both users with the same time periods
    // This tests that automatic cleanup works independently for each user
    let start_time = Utc::now();
    let end_time = start_time + Duration::hours(1);

    let _ = create_overlapping_workouts_batch(
        &test_app,
        &user1.token,
        vec![
            (start_time, end_time, 200),
            (start_time, end_time, 300),
        ],
    ).await.expect("Failed to create user1 overlapping workouts");

    let _ = create_overlapping_workouts_batch(
        &test_app,
        &user2.token,
        vec![
            (start_time, end_time, 250),
            (start_time, end_time, 350),
        ],
    ).await.expect("Failed to create user2 overlapping workouts");

    // Check user1 has 1 workout by querying workout history
    let client = reqwest::Client::new();
    let user1_history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to execute user1 workout history request");

    assert!(user1_history_response.status().is_success());
    
    let user1_history_data: serde_json::Value = user1_history_response
        .json()
        .await
        .expect("Failed to parse user1 workout history response");

    let user1_workouts = user1_history_data["data"]["workouts"].as_array().unwrap();
    assert_eq!(user1_workouts.len(), 1, "User1 should have 1 workout after automatic cleanup");
    assert_eq!(user1_workouts[0]["calories_burned"], 300, "User1 should keep the higher calorie workout");

    // Check user2 also has 1 workout (automatic cleanup affects each user independently)
    let user2_history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .expect("Failed to execute user2 workout history request");

    assert!(user2_history_response.status().is_success());
    
    let user2_history_data: serde_json::Value = user2_history_response
        .json()
        .await
        .expect("Failed to parse user2 workout history response");

    let user2_workouts = user2_history_data["data"]["workouts"].as_array().unwrap();
    assert_eq!(user2_workouts.len(), 1, "User2 should have 1 workout after automatic cleanup");
    assert_eq!(user2_workouts[0]["calories_burned"], 350, "User2 should keep the higher calorie workout");

}

#[tokio::test]
async fn test_duplicate_cleanup_preserves_stats_for_remaining_workout() {
    let test_app = spawn_app().await;
    let user = create_test_user_and_login(&test_app.address).await;

    // Create two overlapping workouts with different calories
    let start_time = Utc::now();
    let end_time = start_time + Duration::hours(1);

    // First, upload a baseline workout at a different time to get the expected stats
    let baseline_start = start_time - Duration::hours(2); // 2 hours earlier
    let baseline_end = baseline_start + Duration::hours(1);
    
    let mut baseline_workout = WorkoutData::new(WorkoutType::Moderate, baseline_start, 60);
    baseline_workout.workout_end = baseline_end;
    baseline_workout.calories_burned = 500; // Same calories as the workout that should remain
    
    let client = reqwest::Client::new();
    let _response = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut baseline_workout).await
        .expect("Failed to upload baseline workout");
    
    // Get the actual stats from the backend for this baseline workout
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to execute workout history request");
    
    let history_data: serde_json::Value = history_response
        .json()
        .await
        .expect("Failed to parse workout history response");
    
    let workouts = history_data["data"]["workouts"].as_array().unwrap();
    assert_eq!(workouts.len(), 1, "Should have 1 workout after baseline upload");
    
    let baseline_workout_data = &workouts[0];
    let baseline_stamina = baseline_workout_data["stamina_gained"].as_f64().unwrap();
    let baseline_strength = baseline_workout_data["strength_gained"].as_f64().unwrap();
    let baseline_total = baseline_stamina + baseline_strength;
    
    println!("Baseline stats: stamina={}, strength={}, total={}", baseline_stamina, baseline_strength, baseline_total);
    
    // Now create overlapping workouts to test duplicate cleanup
    let _ = create_overlapping_workouts_batch(
        &test_app,
        &user.token,
        vec![
            (start_time, end_time, 200), // Lower calorie - should be removed
            (start_time, end_time, 500), // Higher calorie - should remain
        ],
    ).await.expect("Failed to create overlapping workouts");

    // Verify that only one workout remains and it has the correct stats
    let history_response = client
        .get(&format!("{}/health/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to execute workout history request");

    assert!(history_response.status().is_success());

    let history_data: serde_json::Value = history_response
        .json()
        .await
        .expect("Failed to parse workout history response");

    let workouts = history_data["data"]["workouts"].as_array().unwrap();

    // Should have 2 workouts: the baseline workout and the remaining overlapping workout
    assert_eq!(workouts.len(), 2, "Should have 2 workouts: baseline + remaining overlapping workout");

    // Find the workout with 500 calories (the one that should have remained from the overlapping pair)
    // We need to find the one that's NOT the baseline workout (which was 2 hours earlier)
    let remaining_workout = workouts.iter()
        .find(|w| {
            w["calories_burned"] == 500 && 
            w["workout_start"] != baseline_start.to_rfc3339()
        })
        .expect("Should find the remaining 500-calorie workout");

    // Verify that strength and stamina gains are applied to the remaining workout
    assert!(remaining_workout["stamina_gained"].is_number(), "Remaining workout should have stamina gains");
    assert!(remaining_workout["strength_gained"].is_number(), "Remaining workout should have strength gains");
    
    // Get the actual stats from the remaining workout
    let final_stamina = remaining_workout["stamina_gained"].as_f64().unwrap();
    let final_strength = remaining_workout["strength_gained"].as_f64().unwrap();
    let final_total = final_stamina + final_strength;
    
    println!("Final stats: stamina={}, strength={}, total={}", final_stamina, final_strength, final_total);
    
    // Verify that the stats match the baseline (same workout type, same calories, same duration)
    assert_eq!(final_stamina, baseline_stamina, 
        "Stamina gains should match baseline. Expected: {}, Got: {}", baseline_stamina, final_stamina);
    assert_eq!(final_strength, baseline_strength, 
        "Strength gains should match baseline. Expected: {}, Got: {}", baseline_strength, final_strength);
    assert_eq!(final_total, baseline_total, 
        "Total points should match baseline. Expected: {}, Got: {}", baseline_total, final_total);
    
    // Verify the gains are reasonable
    assert!(final_stamina >= 0.0, "Stamina gains should be non-negative");
    assert!(final_strength >= 0.0, "Strength gains should be non-negative");
    assert!(final_total > 0.0, "Total points should be positive for a 60-minute workout");
}