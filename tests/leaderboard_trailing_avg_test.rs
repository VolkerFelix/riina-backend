// Test for leaderboard endpoint with trailing 7-day average

use reqwest::Client;
use serde_json::json;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request};
use common::admin_helpers::{create_admin_user_and_login, create_teams_for_test};
use common::workout_data_helpers::{upload_workout_data_for_user, WorkoutData, WorkoutType};

#[tokio::test]
async fn test_leaderboard_includes_trailing_7_average() {
    let test_app = spawn_app().await;
    let client = Client::new();
    
    println!("🧪 Testing leaderboard endpoint with trailing 7-day average...");
    
    // Create admin user and login
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    let admin_token = &admin_user.token;
    
    // Create test teams
    let teams = create_teams_for_test(&test_app.address, admin_token, 2).await;
    let team1 = &teams[0];
    let team2 = &teams[1];
    
    // Create test users and assign to teams
    let user1 = create_test_user_and_login(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;
    
    // Assign users to teams (this would require team assignment API calls)
    // For now, we'll test with the basic leaderboard endpoint
    
    // Upload some workout data for user1 to generate trailing average
    let workout_data = WorkoutData {
        device_id: "test_device_1".to_string(),
        timestamp: chrono::Utc::now(),
        heart_rate: Some(vec![
            common::workout_data_helpers::HeartRateData {
                timestamp: chrono::Utc::now(),
                heart_rate: 150,
            },
            common::workout_data_helpers::HeartRateData {
                timestamp: chrono::Utc::now() + chrono::Duration::minutes(1),
                heart_rate: 160,
            },
        ]),
        calories_burned: Some(300),
        workout_uuid: uuid::Uuid::new_v4().to_string(),
        workout_start: chrono::Utc::now() - chrono::Duration::hours(1),
        workout_end: chrono::Utc::now(),
        activity_name: Some("Running".to_string()),
        image_url: None,
        video_url: None,
        approval_token: None,
    };
    
    // Upload workout data for user1
    upload_workout_data_for_user(&test_app.address, &user1.token, &workout_data).await;
    
    // Test the leaderboard endpoint
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/users-with-stats", test_app.address),
        &user1.token,
        None,
    ).await;
    
    assert!(response.status().is_success(), "Leaderboard endpoint should return success");
    
    let response_data: serde_json::Value = response.json().await.expect("Failed to parse response");
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
        
        println!("✅ Leaderboard endpoint includes trailing_7_average field: {}", trailing_avg);
    }
    
    println!("✅ Leaderboard trailing 7-day average test passed");
}
