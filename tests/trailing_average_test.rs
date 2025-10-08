// Test for trailing average calculation functions

use reqwest::Client;
use serde_json::json;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request};
use common::workout_data_helpers::{upload_workout_data_for_user, WorkoutData, WorkoutType, create_health_profile_for_user};
use common::admin_helpers::{create_admin_user_and_login, create_teams_for_test};
use riina_backend::db::health_data::get_user_health_profile_details;
use riina_backend::game::stats_calculator::WorkoutStatsCalculator;


#[tokio::test]
async fn test_trailing_7_day_average_calculation() {
    let test_app = spawn_app().await;
    let client = Client::new();
    
    println!("🧪 Testing trailing 7-day average calculation...");
    
    // Create admin user and teams
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    let teams = create_teams_for_test(&test_app.address, &admin_user.token, 1).await;
    let team = &teams[0];
    
    // Create test user and add to team
    let user = create_test_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &user).await.unwrap();
    
    // Add user to team using admin API
    let add_user_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams/{}/members", test_app.address, team),
        &admin_user.token,
        Some(json!({
            "user_id": user.user_id,
            "role": "member"
        })),
    ).await;

    let status = add_user_response.status();
    let response_body = add_user_response.text().await.unwrap_or_default();
    assert!(status.is_success(), "Should successfully add user to team. Status: {}, Body: {}", status, response_body);
    
    let user_health_profile = get_user_health_profile_details(&test_app.db_pool, user.user_id).await.unwrap();

    
    // Upload workout data for the user
    let mut workout_data_vec = Vec::new();
    for i in 0..7 {
        workout_data_vec.push(WorkoutData::new(WorkoutType::Intense, chrono::Utc::now() - chrono::Duration::days(i), 30));
    }

    for mut workout_data in workout_data_vec.iter_mut() {
        let _ = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout_data).await;
    }

    // Get the score for the latest workout
    let workout_stats_calculator = WorkoutStatsCalculator::with_universal_hr_based();
    let workout_stats = workout_stats_calculator.calculate_stat_changes(user_health_profile, workout_data_vec[0].get_heart_rate_data()).await.unwrap();
    let score = &workout_stats.changes.stamina_change + &workout_stats.changes.strength_change;
    
    // Test the leaderboard endpoint to see if trailing average is calculated
    let response = client
        .get(&format!("{}/league/users/stats", test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to call leaderboard API");

    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();
    assert!(status.is_success(), "Leaderboard API should return success. Status: {}, Body: {}", status, response_text);

    let response_data: serde_json::Value = serde_json::from_str(&response_text).expect("Failed to parse response");
    assert_eq!(response_data["success"], true, "API response should indicate success");
    
    // Verify the response structure includes trailing_average
    let data = response_data["data"].as_array().expect("Data should be an array");
    
    if !data.is_empty() {
        let first_user = &data[0];
        
        // Check that trailing_average field exists
        assert!(
            first_user.get("trailing_average").is_some(),
            "Response should include trailing_average field"
        );
        
        // Check that it's a number
        let trailing_avg = first_user.get("trailing_average").unwrap();
        assert!(
            trailing_avg.is_number(),
            "trailing_average should be a number"
        );

        // Check it's the same as the workout stats        
        assert_eq!(trailing_avg, score);
        
        println!("✅ Trailing 7-day average calculation test passed: {}", trailing_avg);
    }
    
    println!("✅ Trailing average calculation test completed");
}

#[tokio::test]
async fn test_leaderboard_sort_by_trailing_average() {
    let test_app = spawn_app().await;
    let client = Client::new();
    
    println!("🧪 Testing leaderboard sorting by trailing average...");
    
    // Create admin user and teams
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    let teams = create_teams_for_test(&test_app.address, &admin_user.token, 1).await;
    let team = &teams[0];
    
    // Create two test users with different workout patterns
    let user1 = create_test_user_and_login(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;
    
    create_health_profile_for_user(&client, &test_app.address, &user1).await.unwrap();
    create_health_profile_for_user(&client, &test_app.address, &user2).await.unwrap();
    
    // Add both users to team
    for user in [&user1, &user2] {
        let add_user_response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/teams/{}/members", test_app.address, team),
            &admin_user.token,
            Some(json!({
                "user_id": user.user_id,
                "role": "member"
            })),
        ).await;

        let status = add_user_response.status();
        let response_body = add_user_response.text().await.unwrap_or_default();
        assert!(status.is_success(), "Should successfully add user to team. Status: {}, Body: {}", status, response_body);
    }
    
    let user1_health_profile = get_user_health_profile_details(&test_app.db_pool, user1.user_id).await.unwrap();
    let user2_health_profile = get_user_health_profile_details(&test_app.db_pool, user2.user_id).await.unwrap();

    // User 1: High intensity workouts in the last 7 days (should have high trailing average)
    let mut user1_workouts = Vec::new();
    for i in 0..7 {
        user1_workouts.push(WorkoutData::new(WorkoutType::Intense, chrono::Utc::now() - chrono::Duration::days(i), 45));
    }

    // User 2: Low intensity workouts in the last 7 days (should have low trailing average)
    let mut user2_workouts = Vec::new();
    for i in 0..7 {
        user2_workouts.push(WorkoutData::new(WorkoutType::Light, chrono::Utc::now() - chrono::Duration::days(i), 20));
    }

    // Upload workout data for both users
    for workout_data in user1_workouts.iter_mut() {
        let _ = upload_workout_data_for_user(&client, &test_app.address, &user1.token, workout_data).await;
    }
    
    for workout_data in user2_workouts.iter_mut() {
        let _ = upload_workout_data_for_user(&client, &test_app.address, &user2.token, workout_data).await;
    }

    // Calculate expected trailing averages
    let workout_stats_calculator = WorkoutStatsCalculator::with_universal_hr_based();
    let user1_stats = workout_stats_calculator.calculate_stat_changes(user1_health_profile, user1_workouts[0].get_heart_rate_data()).await.unwrap();
    let user1_expected_avg = user1_stats.changes.stamina_change + user1_stats.changes.strength_change;
    
    let user2_stats = workout_stats_calculator.calculate_stat_changes(user2_health_profile, user2_workouts[0].get_heart_rate_data()).await.unwrap();
    let user2_expected_avg = user2_stats.changes.stamina_change + user2_stats.changes.strength_change;
    
    // Test leaderboard with sort_by=trailing_average
    let response = client
        .get(&format!("{}/league/users/stats?sort_by=trailing_average", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to call leaderboard API");

    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();
    assert!(status.is_success(), "Leaderboard API should return success. Status: {}, Body: {}", status, response_text);

    let response_data: serde_json::Value = serde_json::from_str(&response_text).expect("Failed to parse response");
    assert_eq!(response_data["success"], true, "API response should indicate success");
    
    let data = response_data["data"].as_array().expect("Data should be an array");
    assert!(!data.is_empty(), "Should have at least one user in the leaderboard");
    
    // Find our test users in the response
    let mut user1_found = None;
    let mut user2_found = None;
    
    for user_data in data {
        let user_id = user_data["user_id"].as_str().unwrap();
        if user_id == user1.user_id.to_string() {
            user1_found = Some(user_data);
        } else if user_id == user2.user_id.to_string() {
            user2_found = Some(user_data);
        }
    }
    
    assert!(user1_found.is_some(), "User 1 should be found in leaderboard");
    assert!(user2_found.is_some(), "User 2 should be found in leaderboard");
    
    let user1_data = user1_found.unwrap();
    let user2_data = user2_found.unwrap();
    
    // Check that trailing averages are calculated correctly
    let user1_trailing_avg = user1_data["trailing_average"].as_f64().unwrap() as f32;
    let user2_trailing_avg = user2_data["trailing_average"].as_f64().unwrap() as f32;
    
    // User 1 should have higher trailing average than User 2
    assert!(user1_trailing_avg > user2_trailing_avg, 
        "User 1 (high intensity) should have higher trailing average than User 2 (low intensity). User1: {}, User2: {}", 
        user1_trailing_avg, user2_trailing_avg);
    
    // Check that the ranking is correct (User 1 should be ranked higher)
    let user1_rank = user1_data["rank"].as_i64().unwrap() as i32;
    let user2_rank = user2_data["rank"].as_i64().unwrap() as i32;
    
    assert!(user1_rank < user2_rank, 
        "User 1 should be ranked higher (lower rank number) than User 2. User1 rank: {}, User2 rank: {}", 
        user1_rank, user2_rank);
    
    println!("✅ User 1 trailing average: {} (rank: {})", user1_trailing_avg, user1_rank);
    println!("✅ User 2 trailing average: {} (rank: {})", user2_trailing_avg, user2_rank);
    println!("✅ Leaderboard sorting by trailing average test passed");
}
