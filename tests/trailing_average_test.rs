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
    
    // Verify the response structure includes trailing_7_average
    let data = response_data["data"].as_array().expect("Data should be an array");
    
    if !data.is_empty() {
        let first_user = &data[0];
        
        // Check that trailing_7_average field exists
        assert!(
            first_user.get("trailing_7_average").is_some(),
            "Response should include trailing_7_average field"
        );
        
        // Check that it's a number
        let trailing_avg = first_user.get("trailing_7_average").unwrap();
        assert!(
            trailing_avg.is_number(),
            "trailing_7_average should be a number"
        );

        // Check it's the same as the workout stats        
        assert_eq!(trailing_avg, score);
        
        println!("✅ Trailing 7-day average calculation test passed: {}", trailing_avg);
    }
    
    println!("✅ Trailing average calculation test completed");
}
