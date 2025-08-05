use actix_web::test;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;
use reqwest::Client;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request};
use common::workout_data_helpers;
use common::admin_helpers::create_admin_user_and_login;

#[tokio::test]
async fn test_admin_can_delete_workout() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a regular user
    let user = create_test_user_and_login(&test_app.address).await;

    // Create a workout for the user using workout data helpers
    let workout_data = workout_data_helpers::create_advanced_workout_data();

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

    // Create multiple workouts
    let workout1 = workout_data_helpers::create_advanced_workout_data();
    let workout2 = workout_data_helpers::create_advanced_workout_data();
    let workout3 = workout_data_helpers::create_advanced_workout_data();

    let workout_response1 = client
        .post(&format!("{}/health/upload_health", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&workout1)
        .send()
        .await
        .expect("Failed to execute health upload request.");

    let workout_response2 = client
        .post(&format!("{}/health/upload_health", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&workout2)
        .send()
        .await
        .expect("Failed to execute health upload request.");

    let workout_response3 = client
        .post(&format!("{}/health/upload_health", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&workout3)
        .send()
        .await
        .expect("Failed to execute health upload request.");

    assert!(workout_response1.status().is_success(), "Workout upload should succeed");
    assert!(workout_response2.status().is_success(), "Workout upload should succeed");
    assert!(workout_response3.status().is_success(), "Workout upload should succeed");

    let response_data1 = workout_response1.json::<serde_json::Value>().await.unwrap();
    let sync_id1 = response_data1["data"]["sync_id"].as_str().unwrap();
    let response_data2 = workout_response2.json::<serde_json::Value>().await.unwrap();
    let sync_id2 = response_data2["data"]["sync_id"].as_str().unwrap();
    let response_data3 = workout_response3.json::<serde_json::Value>().await.unwrap();
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
    let workout_data = workout_data_helpers::create_advanced_workout_data();

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
    let workout1 = workout_data_helpers::create_advanced_workout_data();
    let workout2 = workout_data_helpers::create_advanced_workout_data();

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

    let workout_data = workout_data_helpers::create_advanced_workout_data();

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
    assert_eq!(body["device_id"], workout_data["device_id"].as_str().unwrap());
    assert_eq!(body["calories_burned"], 520);
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
    let workout_data = workout_data_helpers::create_advanced_workout_data();

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