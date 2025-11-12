use reqwest::Client;
use serde_json::json;
use chrono::Utc;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, delete_test_user};
use common::admin_helpers::create_admin_user_and_login;

#[tokio::test]
async fn test_analytics_store_session_events() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create users
    let user = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Prepare session events
    let session_id = format!("session_{}", Utc::now().timestamp_millis());
    let events_payload = json!({
        "events": [
            {
                "event_name": "app_session_start",
                "event_data": {
                    "session_id": session_id.clone()
                },
                "session_id": session_id.clone(),
                "user_hash": "a3f2e8c9d1b4f7e6",
                "timestamp": Utc::now().timestamp_millis(),
                "platform": "ios"
            },
            {
                "event_name": "app_session_end",
                "event_data": {
                    "session_id": session_id.clone(),
                    "duration_ms": 120000,
                    "duration_minutes": 2
                },
                "session_id": session_id,
                "user_hash": "a3f2e8c9d1b4f7e6",
                "timestamp": Utc::now().timestamp_millis(),
                "platform": "ios"
            }
        ]
    });

    // Send analytics events
    let response = client
        .post(&format!("{}/analytics/events", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&events_payload)
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success(), "Analytics events should be stored successfully");

    let body = response.json::<serde_json::Value>()
        .await
        .expect("Failed to parse response body");

    assert_eq!(body["success"], true);
    assert_eq!(body["inserted"], 2);
    assert_eq!(body["total"], 2);

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, user.user_id).await;
}

#[tokio::test]
async fn test_analytics_store_screen_events() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create users
    let user = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address).await;

    let session_id = format!("session_{}", Utc::now().timestamp_millis());
    let events_payload = json!({
        "events": [
            {
                "event_name": "screen_view",
                "event_data": {
                    "screen_name": "NewsfeedScreen"
                },
                "screen_name": "NewsfeedScreen",
                "session_id": session_id.clone(),
                "user_hash": "a3f2e8c9d1b4f7e6",
                "timestamp": Utc::now().timestamp_millis(),
                "platform": "ios"
            },
            {
                "event_name": "screen_exit",
                "event_data": {
                    "screen_name": "NewsfeedScreen",
                    "duration_ms": 5000,
                    "duration_seconds": 5
                },
                "screen_name": "NewsfeedScreen",
                "session_id": session_id,
                "user_hash": "a3f2e8c9d1b4f7e6",
                "timestamp": Utc::now().timestamp_millis(),
                "platform": "ios"
            }
        ]
    });

    let response = client
        .post(&format!("{}/analytics/events", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&events_payload)
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());

    let body = response.json::<serde_json::Value>()
        .await
        .expect("Failed to parse response body");

    assert_eq!(body["success"], true);
    assert_eq!(body["inserted"], 2);

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, user.user_id).await;
}

#[tokio::test]
async fn test_analytics_batch_events() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create users
    let user = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address).await;

    let session_id = format!("session_{}", Utc::now().timestamp_millis());

    // Create a batch of 10 events
    let mut events = Vec::new();
    for i in 0..10 {
        events.push(json!({
            "event_name": "screen_view",
            "event_data": {
                "screen_name": format!("Screen{}", i)
            },
            "screen_name": format!("Screen{}", i),
            "session_id": session_id.clone(),
            "user_hash": "a3f2e8c9d1b4f7e6",
            "timestamp": Utc::now().timestamp_millis() + i * 1000,
            "platform": "ios"
        }));
    }

    let events_payload = json!({
        "events": events
    });

    let response = client
        .post(&format!("{}/analytics/events", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&events_payload)
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());

    let body = response.json::<serde_json::Value>()
        .await
        .expect("Failed to parse response body");

    assert_eq!(body["success"], true);
    assert_eq!(body["inserted"], 10);
    assert_eq!(body["total"], 10);

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, user.user_id).await;
}

#[tokio::test]
async fn test_analytics_requires_authentication() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let events_payload = json!({
        "events": [{
            "event_name": "screen_view",
            "event_data": {
                "screen_name": "TestScreen"
            },
            "timestamp": Utc::now().timestamp_millis(),
            "platform": "ios"
        }]
    });

    // Send without authentication
    let response = client
        .post(&format!("{}/analytics/events", &test_app.address))
        .json(&events_payload)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(response.status().as_u16(), 401, "Should require authentication");
}

#[tokio::test]
async fn test_analytics_validates_event_structure() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create users
    let user = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Invalid: session event with wrong data structure
    let invalid_payload = json!({
        "events": [{
            "event_name": "app_session_start",
            "event_data": {
                "screen_name": "TestScreen"  // Wrong! Should be session_id
            },
            "timestamp": Utc::now().timestamp_millis(),
            "platform": "ios"
        }]
    });

    let response = client
        .post(&format!("{}/analytics/events", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&invalid_payload)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(response.status().as_u16(), 400, "Should reject invalid event structure");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, user.user_id).await;
}

#[tokio::test]
async fn test_analytics_stores_user_hash() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create users
    let user = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address).await;

    let user_hash = "abc123def456";
    let events_payload = json!({
        "events": [{
            "event_name": "screen_view",
            "event_data": {
                "screen_name": "TestScreen"
            },
            "user_hash": user_hash,
            "timestamp": Utc::now().timestamp_millis(),
            "platform": "ios"
        }]
    });

    let response = client
        .post(&format!("{}/analytics/events", &test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&events_payload)
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());

    // Verify the event was stored
    let stored_events = sqlx::query!(
        r#"
        SELECT user_hash, event_name
        FROM analytics_events
        WHERE user_hash = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        user_hash
    )
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to fetch stored event");

    assert_eq!(stored_events.user_hash.as_deref(), Some(user_hash));
    assert_eq!(stored_events.event_name, "screen_view");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, user.user_id).await;
}
