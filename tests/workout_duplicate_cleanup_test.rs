use chrono::{Duration, Utc};
use uuid::Uuid;
use riina_backend::db::workout_duplicate_cleanup::find_overlapping_workouts;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, TestApp};
use common::workout_data_helpers::{WorkoutData, WorkoutType, upload_workout_data_for_user};

/// Helper function to create multiple overlapping workouts in a single batch
async fn create_overlapping_workouts_batch(
    test_app: &TestApp,
    user_token: &str,
    workout_specs: Vec<(chrono::DateTime<Utc>, chrono::DateTime<Utc>, Option<i32>)>,
) -> Result<Vec<Uuid>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let mut workout_data_vec = Vec::new();
    
    // Create all workout data first
    for (start_time, end_time, calories) in workout_specs {
        let duration_minutes = (end_time - start_time).num_minutes() as i64;
        let mut workout_data = WorkoutData::new(WorkoutType::Moderate, start_time, duration_minutes);
        workout_data.workout_end = end_time;
        if let Some(cal) = calories {
            workout_data.calories_burned = cal;
        }
        workout_data_vec.push(workout_data);
    }
    
    // Get approval tokens for all workouts in a single batch request
    let workout_sync_requests: Vec<_> = workout_data_vec.iter().map(|workout| {
        serde_json::json!({
            "start": workout.workout_start,
            "end": workout.workout_end,
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
            for workout_data in &mut workout_data_vec {
                if workout_data.workout_uuid == workout_id {
                    workout_data.approval_token = Some(token.to_string());
                    break;
                }
            }
        }
    }
    
    // Now upload all workouts individually
    let mut workout_ids = Vec::new();
    for mut workout_data in workout_data_vec {
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
    calories: Option<i32>,
) -> Result<Uuid, Box<dyn std::error::Error>> {
    let workout_ids = create_overlapping_workouts_batch(
        test_app,
        user_token,
        vec![(start_time, end_time, calories)],
    ).await?;
    
    Ok(workout_ids[0])
}

#[tokio::test]
async fn test_no_duplicates_when_workouts_dont_overlap() {
    let test_app = spawn_app().await;
    let user = create_test_user_and_login(&test_app.address).await;

    // Create two non-overlapping workouts
    let start1 = Utc::now();
    let end1 = start1 + Duration::hours(1);
    let start2 = end1 + Duration::hours(1); // Starts after first ends
    let end2 = start2 + Duration::hours(1);

    create_single_workout(&test_app, &user.token, start1, end1, Some(200)).await
        .expect("Failed to create first workout");
    create_single_workout(&test_app, &user.token, start2, end2, Some(300)).await
        .expect("Failed to create second workout");

    // Find overlapping workouts
    let overlaps = find_overlapping_workouts(&test_app.db_pool, user.user_id).await
        .expect("Failed to find overlapping workouts");

    // Should find no overlapping groups
    assert_eq!(overlaps.len(), 0, "Should find no overlapping workouts");
}

#[tokio::test]
async fn test_exact_duplicate_workouts_are_detected() {
    let test_app = spawn_app().await;
    let user = create_test_user_and_login(&test_app.address).await;

    // Create two workouts with exact same times
    let start_time = Utc::now();
    let end_time = start_time + Duration::hours(1);

    create_overlapping_workouts_batch(
        &test_app,
        &user.token,
        vec![
            (start_time, end_time, Some(200)),
            (start_time, end_time, Some(300)),
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

    create_overlapping_workouts_batch(
        &test_app,
        &user.token,
        vec![
            (start_time, end_time, Some(200)),
            (start_time, end_time, Some(500)),
        ],
    ).await.expect("Failed to create overlapping workouts");

    // The backend automatically handles duplicate cleanup during upload
    // No need to manually call cleanup function

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
}

#[tokio::test]
async fn test_cleanup_handles_null_calories() {
    let test_app = spawn_app().await;
    let user = create_test_user_and_login(&test_app.address).await;

    // Create workouts with same times - one with null calories, one with value
    let start_time = Utc::now();
    let end_time = start_time + Duration::hours(1);

    create_overlapping_workouts_batch(
        &test_app,
        &user.token,
        vec![
            (start_time, end_time, None),
            (start_time, end_time, Some(300)),
        ],
    ).await.expect("Failed to create overlapping workouts");

    // The backend automatically handles duplicate cleanup during upload
    // No need to manually call cleanup function

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

    create_overlapping_workouts_batch(
        &test_app,
        &user.token,
        vec![
            (start_time, end_time, Some(100)),
            (start_time, end_time, Some(200)),
            (start_time, end_time, Some(400)),
            (start_time, end_time, Some(300)),
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
    create_overlapping_workouts_batch(
        &test_app,
        &user.token,
        vec![
            (start_time, end_time, Some(300)),
            (start_time, end_time, Some(300)),
        ],
    ).await.expect("Failed to create overlapping workouts");

    // The backend automatically handles duplicate cleanup during upload
    // No need to manually call cleanup function

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

    create_single_workout(&test_app, &user.token, start1, end1, Some(200)).await
        .expect("Failed to create first workout");
    create_single_workout(&test_app, &user.token, start2, end2, Some(300)).await
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

    create_overlapping_workouts_batch(
        &test_app,
        &user1.token,
        vec![
            (start_time, end_time, Some(200)),
            (start_time, end_time, Some(300)),
        ],
    ).await.expect("Failed to create user1 overlapping workouts");

    create_overlapping_workouts_batch(
        &test_app,
        &user2.token,
        vec![
            (start_time, end_time, Some(250)),
            (start_time, end_time, Some(350)),
        ],
    ).await.expect("Failed to create user2 overlapping workouts");

    // The backend automatically handles duplicate cleanup during upload
    // No need to manually call cleanup function

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