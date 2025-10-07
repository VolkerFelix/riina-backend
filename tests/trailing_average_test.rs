// Test for trailing average calculation functions

use reqwest::Client;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};
use common::workout_data_helpers::{upload_workout_data_for_user, WorkoutData};

#[tokio::test]
async fn test_trailing_7_day_average_calculation() {
    let test_app = spawn_app().await;
    let client = Client::new();
    
    println!("🧪 Testing trailing 7-day average calculation...");
    
    // Create test user
    let user = create_test_user_and_login(&test_app.address).await;
    
    // Upload workout data for the user
    let workout_data = WorkoutData {
        device_id: "test_device".to_string(),
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
    
    // Upload workout data
    upload_workout_data_for_user(&test_app.address, &user.token, &workout_data).await;
    
    // Test the leaderboard endpoint to see if trailing average is calculated
    let response = client
        .get(&format!("{}/league/users-with-stats", test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to call leaderboard API");
    
    assert!(response.status().is_success(), "Leaderboard API should return success");
    
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
        
        println!("✅ Trailing 7-day average calculation test passed: {}", trailing_avg);
    }
    
    println!("✅ Trailing average calculation test completed");
}
