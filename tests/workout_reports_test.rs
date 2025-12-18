//! Workout reporting functionality tests
//!
//! This test suite covers:
//! - Submitting workout reports for suspicious activity
//! - Retrieving reports by users
//! - Admin management of reports
//! - Deleting reports
//! - Access control for reports

use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use chrono::Utc;
use futures::StreamExt;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request, delete_test_user};
use common::workout_data_helpers::{WorkoutData, WorkoutIntensity, upload_workout_data_for_user, create_health_profile_for_user};
use common::admin_helpers::create_admin_user_and_login;
use common::redis_helpers::setup_redis_pubsub;

// ============================================================================
// WORKOUT REPORT SUBMISSION TESTS
// ============================================================================

#[tokio::test]
async fn test_submit_workout_report() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter = create_test_user_and_login(&test_app.address).await;
    let workout_owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    // Create health profile and upload workout for the owner
    create_health_profile_for_user(&client, &test_app.address, &workout_owner).await.unwrap();
    let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &workout_owner.token, &mut workout_data).await;
    assert!(upload_response.is_ok(), "Workout upload should succeed");
    let upload_result = upload_response.unwrap();
    let workout_id = upload_result["data"]["sync_id"].as_str().unwrap();

    // Reporter submits a report for the workout
    let report_data = json!({
        "reason": "Suspicious heart rate patterns that don't match the reported intensity"
    });

    let response = client
        .post(&format!("{}/health/workout/{}/report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .json(&report_data)
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success(), "Report submission should succeed");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["workout_data_id"].as_str().unwrap(), workout_id);
    assert_eq!(response_body["reported_by_user_id"].as_str().unwrap(), reporter.user_id.to_string());
    assert_eq!(response_body["workout_owner_id"].as_str().unwrap(), workout_owner.user_id.to_string());
    assert_eq!(response_body["reason"].as_str().unwrap(), "Suspicious heart rate patterns that don't match the reported intensity");
    assert_eq!(response_body["status"].as_str().unwrap(), "pending");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter.user_id).await;
    delete_test_user(&test_app.address, &admin.token, workout_owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_submit_report_with_empty_reason_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter = create_test_user_and_login(&test_app.address).await;
    let workout_owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    create_health_profile_for_user(&client, &test_app.address, &workout_owner).await.unwrap();
    let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &workout_owner.token, &mut workout_data).await;
    let upload_result = upload_response.unwrap();
    let workout_id = upload_result["data"]["sync_id"].as_str().unwrap();

    // Try to submit report with empty reason
    let report_data = json!({
        "reason": ""
    });

    let response = client
        .post(&format!("{}/health/workout/{}/report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .json(&report_data)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 400, "Empty reason should be rejected");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter.user_id).await;
    delete_test_user(&test_app.address, &admin.token, workout_owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_submit_report_for_nonexistent_workout_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    let fake_workout_id = Uuid::new_v4();
    let report_data = json!({
        "reason": "Suspicious activity"
    });

    let response = client
        .post(&format!("{}/health/workout/{}/report", &test_app.address, fake_workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .json(&report_data)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 404, "Reporting nonexistent workout should fail");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_update_existing_report() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter = create_test_user_and_login(&test_app.address).await;
    let workout_owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    create_health_profile_for_user(&client, &test_app.address, &workout_owner).await.unwrap();
    let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &workout_owner.token, &mut workout_data).await;
    let upload_result = upload_response.unwrap();
    let workout_id = upload_result["data"]["sync_id"].as_str().unwrap();

    // Submit initial report
    let report_data = json!({
        "reason": "Initial concern"
    });
    client
        .post(&format!("{}/health/workout/{}/report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .json(&report_data)
        .send()
        .await
        .expect("Failed to execute request");

    // Update the report with new reason
    let updated_report_data = json!({
        "reason": "Updated concern with more details"
    });
    let response = client
        .post(&format!("{}/health/workout/{}/report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .json(&updated_report_data)
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success(), "Report update should succeed");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["reason"].as_str().unwrap(), "Updated concern with more details");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter.user_id).await;
    delete_test_user(&test_app.address, &admin.token, workout_owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

// ============================================================================
// RETRIEVE REPORTS TESTS
// ============================================================================

#[tokio::test]
async fn test_get_my_report_for_workout() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter = create_test_user_and_login(&test_app.address).await;
    let workout_owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    create_health_profile_for_user(&client, &test_app.address, &workout_owner).await.unwrap();
    let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &workout_owner.token, &mut workout_data).await;
    let upload_result = upload_response.unwrap();
    let workout_id = upload_result["data"]["sync_id"].as_str().unwrap();

    // Submit a report
    let report_data = json!({
        "reason": "Test report"
    });
    client
        .post(&format!("{}/health/workout/{}/report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .json(&report_data)
        .send()
        .await
        .expect("Failed to execute request");

    // Get the report
    let response = client
        .get(&format!("{}/health/workout/{}/my-report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success(), "Getting report should succeed");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["reason"].as_str().unwrap(), "Test report");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter.user_id).await;
    delete_test_user(&test_app.address, &admin.token, workout_owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_get_my_report_for_workout_with_no_report_returns_null() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter = create_test_user_and_login(&test_app.address).await;
    let workout_owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    create_health_profile_for_user(&client, &test_app.address, &workout_owner).await.unwrap();
    let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &workout_owner.token, &mut workout_data).await;
    let upload_result = upload_response.unwrap();
    let workout_id = upload_result["data"]["sync_id"].as_str().unwrap();

    // Get report without submitting one
    let response = client
        .get(&format!("{}/health/workout/{}/my-report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success(), "Request should succeed");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["report"].is_null(), "Report should be null");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter.user_id).await;
    delete_test_user(&test_app.address, &admin.token, workout_owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_other_user_cannot_see_report() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter = create_test_user_and_login(&test_app.address).await;
    let workout_owner = create_test_user_and_login(&test_app.address).await;
    let other_user = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    create_health_profile_for_user(&client, &test_app.address, &workout_owner).await.unwrap();
    let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &workout_owner.token, &mut workout_data).await;
    let upload_result = upload_response.unwrap();
    let workout_id = upload_result["data"]["sync_id"].as_str().unwrap();

    // Reporter submits a report
    let report_data = json!({
        "reason": "Test report - should not be visible to other users"
    });
    client
        .post(&format!("{}/health/workout/{}/report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .json(&report_data)
        .send()
        .await
        .expect("Failed to execute request");

    // Other user tries to get the report
    let response = client
        .get(&format!("{}/health/workout/{}/my-report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", other_user.token))
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success(), "Request should succeed");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["report"].is_null(), "Other user should not see the report");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter.user_id).await;
    delete_test_user(&test_app.address, &admin.token, workout_owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, other_user.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_get_all_my_reports() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter = create_test_user_and_login(&test_app.address).await;
    let workout_owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    create_health_profile_for_user(&client, &test_app.address, &workout_owner).await.unwrap();

    // Upload two workouts and report both
    for i in 0..2 {
        let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30 + (i * 10));
        let upload_response = upload_workout_data_for_user(&client, &test_app.address, &workout_owner.token, &mut workout_data).await;
        let upload_result = upload_response.unwrap();
        let workout_id = upload_result["data"]["sync_id"].as_str().unwrap();

        let report_data = json!({
            "reason": format!("Report {}", i)
        });
        client
            .post(&format!("{}/health/workout/{}/report", &test_app.address, workout_id))
            .header("Authorization", format!("Bearer {}", reporter.token))
            .json(&report_data)
            .send()
            .await
            .expect("Failed to execute request");
    }

    // Get all reports
    let response = client
        .get(&format!("{}/health/reports/my-reports", &test_app.address))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success(), "Getting all reports should succeed");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["count"].as_u64().unwrap(), 2, "Should have 2 reports");
    assert_eq!(response_body["reports"].as_array().unwrap().len(), 2);

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter.user_id).await;
    delete_test_user(&test_app.address, &admin.token, workout_owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

// ============================================================================
// DELETE REPORT TESTS
// ============================================================================

#[tokio::test]
async fn test_delete_own_report() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter = create_test_user_and_login(&test_app.address).await;
    let workout_owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    create_health_profile_for_user(&client, &test_app.address, &workout_owner).await.unwrap();
    let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &workout_owner.token, &mut workout_data).await;
    let upload_result = upload_response.unwrap();
    let workout_id = upload_result["data"]["sync_id"].as_str().unwrap();

    // Submit a report
    let report_data = json!({
        "reason": "Test report to delete"
    });
    let submit_response = client
        .post(&format!("{}/health/workout/{}/report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .json(&report_data)
        .send()
        .await
        .expect("Failed to execute request");

    let report_body: serde_json::Value = submit_response.json().await.expect("Failed to parse response");
    let report_id = report_body["id"].as_str().unwrap();

    // Delete the report
    let response = client
        .delete(&format!("{}/health/reports/{}", &test_app.address, report_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success(), "Deleting own report should succeed");

    // Verify report is deleted
    let get_response = client
        .get(&format!("{}/health/workout/{}/my-report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .send()
        .await
        .expect("Failed to execute request");

    let get_body: serde_json::Value = get_response.json().await.expect("Failed to parse response");
    assert!(get_body["report"].is_null(), "Report should be deleted");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter.user_id).await;
    delete_test_user(&test_app.address, &admin.token, workout_owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_cannot_delete_other_users_report() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter = create_test_user_and_login(&test_app.address).await;
    let other_user = create_test_user_and_login(&test_app.address).await;
    let workout_owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    create_health_profile_for_user(&client, &test_app.address, &workout_owner).await.unwrap();
    let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &workout_owner.token, &mut workout_data).await;
    let upload_result = upload_response.unwrap();
    let workout_id = upload_result["data"]["sync_id"].as_str().unwrap();

    // Reporter submits a report
    let report_data = json!({
        "reason": "Test report"
    });
    let submit_response = client
        .post(&format!("{}/health/workout/{}/report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .json(&report_data)
        .send()
        .await
        .expect("Failed to execute request");

    let report_body: serde_json::Value = submit_response.json().await.expect("Failed to parse response");
    let report_id = report_body["id"].as_str().unwrap();

    // Other user tries to delete the report
    let response = client
        .delete(&format!("{}/health/reports/{}", &test_app.address, report_id))
        .header("Authorization", format!("Bearer {}", other_user.token))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 404, "Should not be able to delete other user's report");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter.user_id).await;
    delete_test_user(&test_app.address, &admin.token, other_user.user_id).await;
    delete_test_user(&test_app.address, &admin.token, workout_owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

// ============================================================================
// ADMIN REPORT MANAGEMENT TESTS
// ============================================================================

#[tokio::test]
async fn test_admin_get_all_reports() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter1 = create_test_user_and_login(&test_app.address).await;
    let reporter2 = create_test_user_and_login(&test_app.address).await;
    let workout_owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    create_health_profile_for_user(&client, &test_app.address, &workout_owner).await.unwrap();

    // Create two different workouts and have different users report them
    for (i, reporter) in [&reporter1, &reporter2].iter().enumerate() {
        // Space out workouts by 1 hour to avoid sync conflicts
        let workout_start = Utc::now() - chrono::Duration::hours(i as i64 + 1);
        let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, workout_start, 30);
        let upload_response = upload_workout_data_for_user(&client, &test_app.address, &workout_owner.token, &mut workout_data).await;
        let upload_result = upload_response.expect("Failed to upload workout");
        let workout_id = upload_result["data"]["sync_id"].as_str().unwrap();

        let report_data = json!({
            "reason": format!("Report from user {}", i)
        });
        let response = client
            .post(&format!("{}/health/workout/{}/report", &test_app.address, workout_id))
            .header("Authorization", format!("Bearer {}", reporter.token))
            .json(&report_data)
            .send()
            .await
            .expect("Failed to execute request");

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_else(|_| "Unable to read response body".to_string());
            panic!("Failed to submit report for user {}: status={}, body={}", i, status, body);
        }
    }

    // Admin gets all reports
    let response = client
        .get(&format!("{}/admin/workout-reports", &test_app.address))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success(), "Admin should be able to get all reports");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["count"].as_u64().unwrap() >= 2, "Should have at least 2 reports");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter1.user_id).await;
    delete_test_user(&test_app.address, &admin.token, reporter2.user_id).await;
    delete_test_user(&test_app.address, &admin.token, workout_owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_admin_get_pending_reports() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter = create_test_user_and_login(&test_app.address).await;
    let workout_owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    create_health_profile_for_user(&client, &test_app.address, &workout_owner).await.unwrap();
    let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &workout_owner.token, &mut workout_data).await;
    let upload_result = upload_response.unwrap();
    let workout_id = upload_result["data"]["sync_id"].as_str().unwrap();

    // Submit a report
    let report_data = json!({
        "reason": "Pending report"
    });
    client
        .post(&format!("{}/health/workout/{}/report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .json(&report_data)
        .send()
        .await
        .expect("Failed to execute request");

    // Admin gets pending reports
    let response = client
        .get(&format!("{}/admin/workout-reports/pending", &test_app.address))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success(), "Admin should be able to get pending reports");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(response_body["count"].as_u64().unwrap() >= 1, "Should have at least 1 pending report");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter.user_id).await;
    delete_test_user(&test_app.address, &admin.token, workout_owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_admin_update_report_status() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter = create_test_user_and_login(&test_app.address).await;
    let workout_owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    create_health_profile_for_user(&client, &test_app.address, &workout_owner).await.unwrap();
    let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &workout_owner.token, &mut workout_data).await;
    let upload_result = upload_response.unwrap();
    let workout_id = upload_result["data"]["sync_id"].as_str().unwrap();

    // Submit a report
    let report_data = json!({
        "reason": "Test report for admin review"
    });
    let submit_response = client
        .post(&format!("{}/health/workout/{}/report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .json(&report_data)
        .send()
        .await
        .expect("Failed to execute request");

    let report_body: serde_json::Value = submit_response.json().await.expect("Failed to parse response");
    let report_id = report_body["id"].as_str().unwrap();

    // Admin updates the report status
    let update_data = json!({
        "status": "confirmed",
        "admin_notes": "Verified suspicious activity"
    });

    let response = client
        .patch(&format!("{}/admin/workout-reports/{}", &test_app.address, report_id))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&update_data)
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success(), "Admin should be able to update report status");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(response_body["status"].as_str().unwrap(), "confirmed");
    assert_eq!(response_body["admin_notes"].as_str().unwrap(), "Verified suspicious activity");
    assert!(!response_body["reviewed_at"].is_null(), "Should have reviewed_at timestamp");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter.user_id).await;
    delete_test_user(&test_app.address, &admin.token, workout_owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_non_admin_cannot_update_report_status() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter = create_test_user_and_login(&test_app.address).await;
    let workout_owner = create_test_user_and_login(&test_app.address).await;
    let non_admin = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    create_health_profile_for_user(&client, &test_app.address, &workout_owner).await.unwrap();
    let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &workout_owner.token, &mut workout_data).await;
    let upload_result = upload_response.unwrap();
    let workout_id = upload_result["data"]["sync_id"].as_str().unwrap();

    // Submit a report
    let report_data = json!({
        "reason": "Test report"
    });
    let submit_response = client
        .post(&format!("{}/health/workout/{}/report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .json(&report_data)
        .send()
        .await
        .expect("Failed to execute request");

    let report_body: serde_json::Value = submit_response.json().await.expect("Failed to parse response");
    let report_id = report_body["id"].as_str().unwrap();

    // Non-admin tries to update the report
    let update_data = json!({
        "status": "dismissed",
        "admin_notes": "Should not work"
    });

    let response = client
        .patch(&format!("{}/admin/workout-reports/{}", &test_app.address, report_id))
        .header("Authorization", format!("Bearer {}", non_admin.token))
        .json(&update_data)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 403, "Non-admin should not be able to update report status");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter.user_id).await;
    delete_test_user(&test_app.address, &admin.token, workout_owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, non_admin.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_non_admin_cannot_get_all_reports() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let non_admin = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    // Non-admin tries to get all reports
    let response = client
        .get(&format!("{}/admin/workout-reports", &test_app.address))
        .header("Authorization", format!("Bearer {}", non_admin.token))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 403, "Non-admin should not be able to get all reports");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, non_admin.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

// ============================================================================
// NOTIFICATION TESTS
// ============================================================================

#[tokio::test]
async fn test_report_status_update_sends_websocket_notification() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let reporter = create_test_user_and_login(&test_app.address).await;
    let workout_owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    // Subscribe to the reporter's WebSocket notification channel BEFORE the event
    let user_channel = format!("game:events:user:{}", reporter.user_id);
    let mut pubsub = setup_redis_pubsub(&user_channel).await;

    // Create workout and submit report
    create_health_profile_for_user(&client, &test_app.address, &workout_owner).await.unwrap();
    let mut workout_data = WorkoutData::new(WorkoutIntensity::Intense, Utc::now(), 30);
    let upload_response = upload_workout_data_for_user(&client, &test_app.address, &workout_owner.token, &mut workout_data).await;
    let upload_result = upload_response.unwrap();
    let workout_id = upload_result["data"]["sync_id"].as_str().unwrap();

    let report_data = json!({
        "reason": "Suspicious activity detected"
    });
    let submit_response = client
        .post(&format!("{}/health/workout/{}/report", &test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", reporter.token))
        .json(&report_data)
        .send()
        .await
        .expect("Failed to execute request");

    let report_body: serde_json::Value = submit_response.json().await.expect("Failed to parse response");
    let report_id = report_body["id"].as_str().unwrap();

    // Admin confirms the report - should trigger WebSocket notification
    let update_data = json!({
        "status": "confirmed",
        "admin_notes": "Verified after investigation"
    });

    let update_response = client
        .patch(&format!("{}/admin/workout-reports/{}", &test_app.address, report_id))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&update_data)
        .send()
        .await
        .expect("Failed to execute request");

    assert!(update_response.status().is_success(), "Report update should succeed");

    let update_body: serde_json::Value = update_response.json().await.expect("Failed to parse response");
    assert_eq!(update_body["status"].as_str().unwrap(), "confirmed");

    // Listen for WebSocket notification event
    let mut stream = pubsub.on_message();

    // Wait for the message with a timeout
    let notification_received = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        async {
            while let Some(msg) = stream.next().await {
                let payload: String = msg.get_payload().expect("Failed to get payload");
                let event: serde_json::Value = serde_json::from_str(&payload).expect("Failed to parse event");

                // Check if this is the notification event
                if event["event_type"] == "notification_received" {
                    return Some(event);
                }
            }
            None
        }
    ).await;

    assert!(notification_received.is_ok(), "Should receive WebSocket notification within timeout");
    let event = notification_received.unwrap();
    assert!(event.is_some(), "WebSocket notification event should be received");

    let notification_event = event.unwrap();
    assert_eq!(notification_event["recipient_id"].as_str().unwrap(), reporter.user_id.to_string());
    assert_eq!(notification_event["notification_type"].as_str().unwrap(), "workout_report");
    assert_eq!(notification_event["actor_username"].as_str().unwrap(), admin.username);

    let message = notification_event["message"].as_str().unwrap();
    assert!(message.contains("Reported Issue Confirmed"), "Message should contain confirmation title");
    assert!(message.contains("Thank you for helping maintain fair play"), "Message should contain confirmation body");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, reporter.user_id).await;
    delete_test_user(&test_app.address, &admin.token, workout_owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}
