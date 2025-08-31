use actix_web::test;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;
use reqwest::Client;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request};
use common::workout_data_helpers::{WorkoutData, WorkoutType, upload_workout_data_for_user};
use common::admin_helpers::create_admin_user_and_login;

#[tokio::test]
async fn test_admin_can_delete_workout() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a regular user
    let user = create_test_user_and_login(&test_app.address).await;

    // Create a workout for the user using workout data helpers
    let mut workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);

    let workout_response = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout_data).await;

    let response_data = workout_response.unwrap();
    let sync_id = response_data["data"]["sync_id"].as_str().unwrap();

    // Admin deletes the workout
    let delete_response = client
        .delete(&format!("{}/admin/workouts/{}", &test_app.address, sync_id))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute workout delete request.");

    assert!(delete_response.status().is_success(), "Workout delete should succeed");

    // Verify workout is deleted
    let get_response = client
        .get(&format!("{}/admin/workouts/{}", &test_app.address, sync_id))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute workout get request.");

    assert_eq!(get_response.status(), 404);
}

#[tokio::test]
async fn test_admin_can_bulk_delete_workouts() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a regular user
    let user = create_test_user_and_login(&test_app.address).await;

    // Create multiple workouts with different timestamps to avoid duplicate detection
    let now = Utc::now();
    let mut workout1 = WorkoutData::new(WorkoutType::Moderate, now - chrono::Duration::hours(2), 30);
    let mut workout2 = WorkoutData::new(WorkoutType::Moderate, now - chrono::Duration::hours(1), 30);
    let mut workout3 = WorkoutData::new(WorkoutType::Moderate, now, 30);

    let workout_response1 = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout1).await;
    let workout_response2 = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout2).await;
    let workout_response3 = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout3).await;

    assert!(workout_response1.is_ok(), "Workout upload should succeed");
    assert!(workout_response2.is_ok(), "Workout upload should succeed");
    assert!(workout_response3.is_ok(), "Workout upload should succeed");

    let response_data1 = workout_response1.unwrap();
    let sync_id1 = response_data1["data"]["sync_id"].as_str().unwrap();
    let response_data2 = workout_response2.unwrap();
    let sync_id2 = response_data2["data"]["sync_id"].as_str().unwrap();
    let response_data3 = workout_response3.unwrap();
    let sync_id3 = response_data3["data"]["sync_id"].as_str().unwrap();

    // Admin bulk deletes workouts 1 and 2
    let delete_response = client
        .post(&format!("{}/admin/workouts/bulk-delete", &test_app.address))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "workout_ids": [sync_id1, sync_id2]
        }))
        .send()
        .await
        .expect("Failed to execute workout bulk delete request.");

    assert!(delete_response.status().is_success(), "Workout bulk delete should succeed. Status: {}", delete_response.status());
    
    let body: serde_json::Value = delete_response.json().await.unwrap();
    assert_eq!(body["deleted_count"], 2);

    // Verify workouts 1 and 2 are deleted
    let get_response1 = client
        .get(&format!("{}/admin/workouts/{}", &test_app.address, sync_id1))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute workout get request.");
    assert_eq!(get_response1.status(), 404);

    let get_response2 = client
        .get(&format!("{}/admin/workouts/{}", &test_app.address, sync_id2))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute workout get request.");
    assert_eq!(get_response2.status(), 404);

    // Verify workout 3 still exists
    let get_response3 = client
        .get(&format!("{}/admin/workouts/{}", &test_app.address, sync_id3))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute workout get request.");
    assert_eq!(get_response3.status(), 200);
}

#[tokio::test]
async fn test_admin_delete_nonexistent_workout_returns_404() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    let fake_id = Uuid::new_v4();

    let delete_response = client
        .delete(&format!("{}/admin/workouts/{}", &test_app.address, fake_id))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute workout delete request.");

    assert_eq!(delete_response.status(), 404);
}

#[tokio::test]
async fn test_non_admin_cannot_delete_workouts() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a regular user and get their token
    let user = create_test_user_and_login(&test_app.address).await;

    // Create a workout
    let workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);

    let workout_response = client
        .post(&format!("{}/health/upload_health", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&workout_data)
        .send()
        .await
        .expect("Failed to execute health upload request.");

    assert!(workout_response.status().is_success(), "Workout upload should succeed");

    let response_data = workout_response.json::<serde_json::Value>().await.unwrap();
    let sync_id = response_data["data"]["sync_id"].as_str().unwrap();

    // Regular user tries to delete workout
    let delete_response = client
        .delete(&format!("{}/admin/workouts/{}", &test_app.address, sync_id))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to execute workout delete request.");

    assert_eq!(delete_response.status(), 401); // Unauthorized - admin routes return 401 for non-admin users

    // Verify workout still exists
    let get_response = client
        .get(&format!("{}/admin/workouts/{}", &test_app.address, sync_id))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute workout get request.");

    assert_eq!(get_response.status(), 200);
}

#[tokio::test]
async fn test_admin_can_list_workouts() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create users
    let user1 = create_test_user_and_login(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // Create workouts for both users
    let workout1 = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);
    let workout2 = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);

    let workout_response1 = client
        .post(&format!("{}/health/upload_health", &test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .json(&workout1)
        .send()
        .await
        .expect("Failed to execute health upload request.");

    let workout_response2 = client
        .post(&format!("{}/health/upload_health", &test_app.address))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&workout2)
        .send()
        .await
        .expect("Failed to execute health upload request.");

    assert!(workout_response1.status().is_success(), "Workout upload should succeed");
    assert!(workout_response2.status().is_success(), "Workout upload should succeed");

    let response_data1 = workout_response1.json::<serde_json::Value>().await.unwrap();
    let sync_id1 = response_data1["data"]["sync_id"].as_str().unwrap();
    let response_data2 = workout_response2.json::<serde_json::Value>().await.unwrap();
    // Admin lists all workouts
    let list_response = client
        .get(&format!("{}/admin/workouts?limit=10", &test_app.address))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute workout list request.");

    assert_eq!(list_response.status(), 200);

    let body: serde_json::Value = list_response.json().await.unwrap();
    assert!(body["workouts"].is_array());
    assert!(body["workouts"].as_array().unwrap().len() >= 2);
    assert!(body["total"].as_i64().unwrap() >= 2);

    // Test filtering by user
    let list_response = client
        .get(&format!("{}/admin/workouts?user_id={}", &test_app.address, user1.user_id))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute workout list request.");

    assert_eq!(list_response.status(), 200);

    let body: serde_json::Value = list_response.json().await.unwrap();
    assert_eq!(body["workouts"].as_array().unwrap().len(), 1);
    assert_eq!(body["workouts"][0]["user_id"], user1.user_id.to_string());
}

#[tokio::test]
async fn test_admin_can_view_workout_details() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a user and workout
    let user = create_test_user_and_login(&test_app.address).await;

    let workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);

    let workout_response = client
        .post(&format!("{}/health/upload_health", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&workout_data)
        .send()
        .await
        .expect("Failed to execute health upload request.");

    assert!(workout_response.status().is_success(), "Workout upload should succeed");

    let response_data = workout_response.json::<serde_json::Value>().await.unwrap();
    let sync_id1 = response_data["data"]["sync_id"].as_str().unwrap();

    // Admin views workout details
    let get_response = client
        .get(&format!("{}/admin/workouts/{}", &test_app.address, sync_id1))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute workout get request.");

    assert_eq!(get_response.status(), 200);

    let body: serde_json::Value = get_response.json().await.unwrap();
    // The workout detail endpoint returns the workout directly, not wrapped
    assert_eq!(body["id"], sync_id1);
    assert_eq!(body["user_id"], user.user_id.to_string());
    assert_eq!(body["username"], user.username);
    assert_eq!(body["device_id"], workout_data.device_id);
    assert_eq!(body["calories_burned"], 300);
    assert!(body["heart_rate"].is_array());
    assert!(body["heart_rate"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn test_workout_cascade_deletes_with_user() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a user
    let user = create_test_user_and_login(&test_app.address).await;

    // Create workouts
    let mut workout_data = WorkoutData::new(WorkoutType::Moderate, Utc::now(), 30);


    let workout_response = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout_data).await;

    assert!(workout_response.is_ok(), "Workout upload should succeed");

    let response_data = workout_response.unwrap();
    let sync_id1 = response_data["data"]["sync_id"].as_str().unwrap();

    // Delete the user
    let delete_response = client
        .delete(&format!("{}/admin/users/{}", &test_app.address, user.user_id))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute user delete request.");

    assert!(delete_response.status().is_success(), "User delete should succeed");

    // Verify workout is also deleted (cascade)
    let get_response = client
        .get(&format!("{}/admin/workouts/{}", &test_app.address, sync_id1))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute workout get request.");

    assert_eq!(get_response.status(), 404);
}

#[tokio::test]
async fn test_workout_deletion_reverses_user_stats() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a regular user
    let user = create_test_user_and_login(&test_app.address).await;

    // Get initial user stats
    let initial_stats_response = client
        .get(&format!("{}/profile/user", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get initial user stats");
    
    assert!(initial_stats_response.status().is_success());
    let initial_stats: serde_json::Value = initial_stats_response.json().await.unwrap();
    let initial_stamina = initial_stats["data"]["stats"]["stamina"].as_i64().unwrap_or(0);
    let initial_strength = initial_stats["data"]["stats"]["strength"].as_i64().unwrap_or(0);
    
    println!("Initial stats - Stamina: {}, Strength: {}", initial_stamina, initial_strength);

    // Upload a workout (this should increase user stats)
    let workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 45);

    let workout_response = client
        .post(&format!("{}/health/upload_health", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&workout_data)
        .send()
        .await
        .expect("Failed to execute health upload request.");

    assert!(workout_response.status().is_success(), "Workout upload should succeed");

    let response_data = workout_response.json::<serde_json::Value>().await.unwrap();
    let sync_id = response_data["data"]["sync_id"].as_str().unwrap();
    
    // Extract the stat changes from the workout response
    let stamina_gained = response_data["data"]["game_stats"]["stamina_change"].as_i64().unwrap_or(0);
    let strength_gained = response_data["data"]["game_stats"]["strength_change"].as_i64().unwrap_or(0);
    
    println!("Workout gave - Stamina: {}, Strength: {}", stamina_gained, strength_gained);
    
    // Verify stats increased
    let after_workout_stats_response = client
        .get(&format!("{}/profile/user", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get user stats after workout");
    
    assert!(after_workout_stats_response.status().is_success());
    let after_workout_stats: serde_json::Value = after_workout_stats_response.json().await.unwrap();
    let after_stamina = after_workout_stats["data"]["stats"]["stamina"].as_i64().unwrap_or(0);
    let after_strength = after_workout_stats["data"]["stats"]["strength"].as_i64().unwrap_or(0);
    
    println!("After workout stats - Stamina: {}, Strength: {}", after_stamina, after_strength);
    
    // Verify stats increased
    assert_eq!(after_stamina, initial_stamina + stamina_gained, "Stamina should have increased by workout amount");
    assert_eq!(after_strength, initial_strength + strength_gained, "Strength should have increased by workout amount");

    // Admin deletes the workout
    let delete_response = client
        .delete(&format!("{}/admin/workouts/{}", &test_app.address, sync_id))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute workout delete request.");

    assert!(delete_response.status().is_success(), "Workout delete should succeed");

    // Verify stats were decreased back to original levels
    let final_stats_response = client
        .get(&format!("{}/profile/user", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get final user stats");
    
    assert!(final_stats_response.status().is_success());
    let final_stats: serde_json::Value = final_stats_response.json().await.unwrap();
    let final_stamina = final_stats["data"]["stats"]["stamina"].as_i64().unwrap_or(0);
    let final_strength = final_stats["data"]["stats"]["strength"].as_i64().unwrap_or(0);
    
    println!("Final stats - Stamina: {}, Strength: {}", final_stamina, final_strength);
    
    // Verify stats returned to original levels (or as close as possible with GREATEST(0, x))
    assert_eq!(final_stamina, initial_stamina, "Stamina should return to original level after workout deletion");
    assert_eq!(final_strength, initial_strength, "Strength should return to original level after workout deletion");
}

#[tokio::test]
async fn test_bulk_workout_deletion_reverses_user_stats() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a regular user
    let user = create_test_user_and_login(&test_app.address).await;

    // Get initial user stats
    let initial_stats_response = client
        .get(&format!("{}/profile/user", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get initial user stats");
    
    assert!(initial_stats_response.status().is_success());
    let initial_stats: serde_json::Value = initial_stats_response.json().await.unwrap();
    let initial_stamina = initial_stats["data"]["stats"]["stamina"].as_i64().unwrap_or(0);
    let initial_strength = initial_stats["data"]["stats"]["strength"].as_i64().unwrap_or(0);
    
    println!("Initial stats - Stamina: {}, Strength: {}", initial_stamina, initial_strength);

    // Upload multiple workouts
    let mut workout_ids = Vec::new();
    let mut total_stamina_gained = 0;
    let mut total_strength_gained = 0;
    
    for i in 0..3 {
        let workout_type = match i {
            0 => WorkoutType::Intense,
            1 => WorkoutType::Moderate,
            _ => WorkoutType::Light,
        };
        
        let workout_data = WorkoutData::new(workout_type, Utc::now(), 30);

        let workout_response = client
            .post(&format!("{}/health/upload_health", &test_app.address))
            .header("Authorization", format!("Bearer {}", user.token))
            .json(&workout_data)
            .send()
            .await
            .expect("Failed to execute health upload request.");

        assert!(workout_response.status().is_success(), "Workout upload should succeed");

        let response_data = workout_response.json::<serde_json::Value>().await.unwrap();
        let sync_id = response_data["data"]["sync_id"].as_str().unwrap();
        workout_ids.push(sync_id.to_string());
        
        // Extract the stat changes from the workout response
        let stamina_gained = response_data["data"]["game_stats"]["stamina_change"].as_i64().unwrap_or(0);
        let strength_gained = response_data["data"]["game_stats"]["strength_change"].as_i64().unwrap_or(0);
        
        total_stamina_gained += stamina_gained;
        total_strength_gained += strength_gained;
        
        println!("Workout {} gave - Stamina: {}, Strength: {}", i + 1, stamina_gained, strength_gained);
    }
    
    // Verify stats increased after all workouts
    let after_workouts_stats_response = client
        .get(&format!("{}/profile/user", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get user stats after workouts");
    
    assert!(after_workouts_stats_response.status().is_success());
    let after_workouts_stats: serde_json::Value = after_workouts_stats_response.json().await.unwrap();
    let after_stamina = after_workouts_stats["data"]["stats"]["stamina"].as_i64().unwrap_or(0);
    let after_strength = after_workouts_stats["data"]["stats"]["strength"].as_i64().unwrap_or(0);
    
    println!("After all workouts stats - Stamina: {}, Strength: {}", after_stamina, after_strength);
    println!("Total gains - Stamina: {}, Strength: {}", total_stamina_gained, total_strength_gained);
    
    // Verify stats increased
    assert_eq!(after_stamina, initial_stamina + total_stamina_gained, "Stamina should have increased by total workout amount");
    assert_eq!(after_strength, initial_strength + total_strength_gained, "Strength should have increased by total workout amount");

    // Admin bulk deletes all workouts
    let delete_response = client
        .post(&format!("{}/admin/workouts/bulk-delete", &test_app.address))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "workout_ids": workout_ids
        }))
        .send()
        .await
        .expect("Failed to execute bulk workout delete request.");

    assert!(delete_response.status().is_success(), "Bulk workout delete should succeed");

    // Verify stats were decreased back to original levels
    let final_stats_response = client
        .get(&format!("{}/profile/user", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get final user stats");
    
    assert!(final_stats_response.status().is_success());
    let final_stats: serde_json::Value = final_stats_response.json().await.unwrap();
    let final_stamina = final_stats["data"]["stats"]["stamina"].as_i64().unwrap_or(0);
    let final_strength = final_stats["data"]["stats"]["strength"].as_i64().unwrap_or(0);
    
    println!("Final stats after bulk deletion - Stamina: {}, Strength: {}", final_stamina, final_strength);
    
    // Verify stats returned to original levels
    assert_eq!(final_stamina, initial_stamina, "Stamina should return to original level after bulk workout deletion");
    assert_eq!(final_strength, initial_strength, "Strength should return to original level after bulk workout deletion");
}