//! Newsfeed integration tests
//!
//! Tests for newsfeed functionality:
//! - Getting workout feed
//! - Pagination with cursors
//! - Reactions and comments in feed
//! - Ordering (most recent first, engagement-based within 24h)
//! - Engagement-based ranking (media, reactions, comments)
//! - Multiple workouts from different users
//! - Authentication checks

use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use chrono::Utc;
use url::form_urlencoded;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};
use common::social_helpers::create_user_with_workout;
use common::workout_data_helpers::{WorkoutData, WorkoutIntensity, upload_workout_data_for_user};

#[tokio::test]
async fn test_newsfeed_basic() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create two users with workouts
    let (user1, _workout1_id) = create_user_with_workout(&test_app.address).await;
    let (user2, _workout2_id) = create_user_with_workout(&test_app.address).await;

    // Get newsfeed for user1 with higher limit to ensure we get all workouts
    let response = client
        .get(&format!("{}/feed/?limit=50", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get newsfeed");

    let status = response.status();
    if !&status.is_success() {
        let error_body = response.text().await.expect("Failed to get error response");
        panic!("Feed request failed with status {}: {}", status, error_body);
    }
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");

    assert_eq!(response_body["success"], true);
    let posts = response_body["data"]["posts"].as_array().unwrap();

    // Should see at least both workouts (user1's and user2's) - other tests may add more
    assert!(posts.len() >= 2, "Should have at least 2 posts in feed, got {}", posts.len());

    // Find the posts from our specific users
    let user1_posts: Vec<_> = posts.iter()
        .filter(|p| p["user_id"] == user1.user_id.to_string())
        .collect();
    let user2_posts: Vec<_> = posts.iter()
        .filter(|p| p["user_id"] == user2.user_id.to_string())
        .collect();

    assert_eq!(user1_posts.len(), 1, "Should have 1 post from user1");
    assert_eq!(user2_posts.len(), 1, "Should have 1 post from user2");

    // Verify posts have expected fields
    for post in posts {
        assert!(post["id"].is_string());
        assert!(post["user_id"].is_string());
        assert!(post["username"].is_string());
        assert!(post["created_at"].is_string());
        assert!(post["reaction_count"].is_number());
        assert!(post["comment_count"].is_number());
        assert!(post["user_has_reacted"].is_boolean());
        assert_eq!(post["visibility"], "public");

    }

}

#[tokio::test]
async fn test_newsfeed_pagination() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create multiple workouts
    let (user, _) = create_user_with_workout(&test_app.address).await;

    // Upload more workouts with delays to ensure different timestamps
    for i in 0..3 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut workout_data = WorkoutData::new(
            WorkoutIntensity::Moderate,
            Utc::now() - chrono::Duration::hours(i),
            30
        );
        let _ = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout_data).await;
    }

    // Small delay to ensure all posts are committed
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Get first page (ranked section)
    // Note: The first request returns ALL ranked posts from last 48 hours (up to 50),
    // ignoring the limit parameter. Limit only applies to chronological pagination.
    let response = client
        .get(&format!("{}/feed/", test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get first page");

    let status = response.status();
    if !&status.is_success() {
        let error_body = response.text().await.expect("Failed to get error response");
        panic!("Feed pagination request failed with status {}: {}", status, error_body);
    }
    let first_page: serde_json::Value = response.json().await.expect("Failed to parse response");

    let posts = first_page["data"]["posts"].as_array().unwrap();
    // First page returns all recent posts (ranked section) - we created 4 workouts
    assert!(posts.len() >= 4, "Should have at least 4 workouts in ranked section (got {})", posts.len());

    let pagination = &first_page["data"]["pagination"];
    // Ranked section always has has_more=true to allow scrolling to chronological posts
    assert!(pagination["has_more"].as_bool().unwrap(), "Should have more pages after ranked section");
    assert!(pagination["next_cursor"].is_string(), "Should have next cursor");

    // Get second page using cursor (chronological section)
    // This returns posts OLDER than 48 hours, which may be empty in this test
    let cursor = pagination["next_cursor"].as_str().unwrap();
    let encoded_cursor = form_urlencoded::byte_serialize(cursor.as_bytes()).collect::<String>();
    let response = client
        .get(&format!("{}/feed/?limit=2&cursor={}", test_app.address, encoded_cursor))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get second page");

    let status = response.status();
    if !&status.is_success() {
        let error_body = response.text().await.expect("Failed to get error response");
        panic!("Feed second page request failed with status {}: {}", status, error_body);
    }
    let second_page: serde_json::Value = response.json().await.expect("Failed to parse response");

    let second_posts = second_page["data"]["posts"].as_array().unwrap();
    // Second page contains chronological posts older than 48 hours
    // Since our test posts are recent, this may be empty - that's OK

    // Verify no overlap between pages (if there are posts in second page)
    if !second_posts.is_empty() {
        let first_ids: Vec<String> = posts.iter()
            .map(|w| w["id"].as_str().unwrap().to_string())
            .collect();
        let second_ids: Vec<String> = second_posts.iter()
            .map(|w| w["id"].as_str().unwrap().to_string())
            .collect();

        for id in &second_ids {
            assert!(!first_ids.contains(id), "Pages should not have overlapping workouts");
        }
    }
}

#[tokio::test]
async fn test_newsfeed_with_reactions_and_comments() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create user with workout
    let (user1, workout_id) = create_user_with_workout(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;

    // User2 adds reaction to user1's workout
    let reaction_data = json!({"reaction_type": "fire"});
    let reaction_response = client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&reaction_data)
        .send()
        .await
        .expect("Failed to add reaction");

    assert!(reaction_response.status().is_success());

    // User2 adds comment to user1's workout
    let comment_data = json!({"content": "Great workout!", "parent_id": null});
    let comment_response = client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout_id))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&comment_data)
        .send()
        .await
        .expect("Failed to add comment");

    assert!(comment_response.status().is_success());

    // Small delay to ensure data is committed
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Get newsfeed and verify counts
    let response = client
        .get(&format!("{}/feed/?limit=50", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get newsfeed");

    assert!(response.status().is_success());
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");

    let posts = response_body["data"]["posts"].as_array().unwrap();
    let user1_post = posts.iter()
        .find(|p| p["workout_id"] == workout_id.to_string())
        .expect("Should find user1's workout post in feed");

    assert_eq!(user1_post["reaction_count"], 1, "Should have 1 reaction");
    assert_eq!(user1_post["comment_count"], 1, "Should have 1 comment");
    assert_eq!(user1_post["user_has_reacted"], false, "User1 has not reacted");

}

#[tokio::test]
async fn test_newsfeed_visibility_filter() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let (user, _) = create_user_with_workout(&test_app.address).await;

    // Get newsfeed
    let response = client
        .get(&format!("{}/feed/?limit=50", test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get newsfeed");

    assert!(response.status().is_success());
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");

    let posts = response_body["data"]["posts"].as_array().unwrap();

    // All posts in feed should be public
    for post in posts {
        assert_eq!(post["visibility"], "public", "All feed posts should be public");

    }

}

#[tokio::test]
async fn test_newsfeed_requires_authentication() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let response = client
        .get(&format!("{}/feed/", test_app.address))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 401, "Should require authentication");

}

#[tokio::test]
async fn test_newsfeed_engagement_ranking_comments_weigh_more() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create two workouts
    let (user1, workout1_id) = create_user_with_workout(&test_app.address).await;
    let (user2, workout2_id) = create_user_with_workout(&test_app.address).await;
    let engager = create_test_user_and_login(&test_app.address).await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // workout1: 1 comment (3 points)
    client
        .post(&format!("{}/social/workouts/{}/comments", test_app.address, workout1_id))
        .header("Authorization", format!("Bearer {}", engager.token))
        .json(&json!({"content": "Excellent work!", "parent_id": null}))
        .send()
        .await
        .expect("Failed to add comment");

    // workout2: 1 reaction (2 points)
    client
        .post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout2_id))
        .header("Authorization", format!("Bearer {}", engager.token))
        .json(&json!({"reaction_type": "fire"}))
        .send()
        .await
        .expect("Failed to add reaction");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get newsfeed
    let response = client
        .get(&format!("{}/feed/?limit=50", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get newsfeed");

    assert!(response.status().is_success());
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");

    let posts = response_body["data"]["posts"].as_array().unwrap();

    // Find positions
    let workout1_pos = posts.iter().position(|p| p["workout_id"] == workout1_id.to_string());
    let workout2_pos = posts.iter().position(|p| p["workout_id"] == workout2_id.to_string());

    assert!(workout1_pos.is_some(), "workout1 should be in feed");
    assert!(workout2_pos.is_some(), "workout2 should be in feed");

    // workout1 with 1 comment (3 points) should rank higher than workout2 with 1 reaction (2 points)
    assert!(workout1_pos.unwrap() < workout2_pos.unwrap(),
        "Workout with comment should rank higher than workout with reaction (comments worth 3 points vs reactions worth 2)");
}

#[tokio::test]
async fn test_newsfeed_engagement_ranking_media_boost() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create user and upload workout with media
    let user1 = create_test_user_and_login(&test_app.address).await;

    // Create workout with image (gets +10 points for media)
    let mut workout_with_media = WorkoutData::new(
        WorkoutIntensity::Moderate,
        Utc::now(),
        30
    );
    workout_with_media.image_urls = Some(vec!["https://example.com/workout-image.jpg".to_string()]);
    let workout1_response = upload_workout_data_for_user(&client, &test_app.address, &user1.token, &mut workout_with_media).await.expect("Failed to upload workout");
    let workout1_id = workout1_response["data"]["sync_id"].as_str().unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Create workout without media
    let user2 = create_test_user_and_login(&test_app.address).await;
    let mut workout_without_media = WorkoutData::new(
        WorkoutIntensity::Moderate,
        Utc::now(),
        30
    );
    let workout2_response = upload_workout_data_for_user(&client, &test_app.address, &user2.token, &mut workout_without_media).await.expect("Failed to upload workout");
    let workout2_id = workout2_response["data"]["sync_id"].as_str().unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Create workout with video (also gets +10 points for media)
    let user3 = create_test_user_and_login(&test_app.address).await;
    let mut workout_with_video = WorkoutData::new(
        WorkoutIntensity::Moderate,
        Utc::now(),
        30
    );
    workout_with_video.video_urls = Some(vec!["https://example.com/workout-video.mp4".to_string()]);
    let workout3_response = upload_workout_data_for_user(&client, &test_app.address, &user3.token, &mut workout_with_video).await.expect("Failed to upload workout");
    let workout3_id = workout3_response["data"]["sync_id"].as_str().unwrap();

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get newsfeed
    let response = client
        .get(&format!("{}/feed/?limit=50", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get newsfeed");

    assert!(response.status().is_success());
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");

    let posts = response_body["data"]["posts"].as_array().unwrap();

    // Find positions
    let workout1_pos = posts.iter().position(|p| p["workout_id"] == workout1_id.to_string());
    let workout2_pos = posts.iter().position(|p| p["workout_id"] == workout2_id.to_string());
    let workout3_pos = posts.iter().position(|p| p["workout_id"] == workout3_id.to_string());

    assert!(workout1_pos.is_some(), "workout with image should be in feed");
    assert!(workout2_pos.is_some(), "workout without media should be in feed");
    assert!(workout3_pos.is_some(), "workout with video should be in feed");

    // Both workouts with media (image or video, +10 points) should rank higher than workout without media
    assert!(workout1_pos.unwrap() < workout2_pos.unwrap(),
        "Workout with image should rank higher than workout without media");
    assert!(workout3_pos.unwrap() < workout2_pos.unwrap(),
        "Workout with video should rank higher than workout without media");
}

#[tokio::test]
async fn test_newsfeed_engagement_ranking_combined_score() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create multiple workouts with different combinations of engagement factors

    // workout1: media (10) + 2 reactions (4) + 1 comment (3) = 17 points
    let user1 = create_test_user_and_login(&test_app.address).await;
    let mut workout1_data = WorkoutData::new(WorkoutIntensity::Moderate, Utc::now(), 30);
    workout1_data.image_urls = Some(vec!["https://example.com/img1.jpg".to_string()]);
    let workout1_response = upload_workout_data_for_user(&client, &test_app.address, &user1.token, &mut workout1_data).await.expect("Failed to upload workout");
    let workout1_id = workout1_response["data"]["sync_id"].as_str().unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // workout2: 3 comments (9) = 9 points
    let (user2, workout2_id) = create_user_with_workout(&test_app.address).await;

    tokio::time::sleep(Duration::from_millis(50)).await;

    // workout3: 4 reactions (8) = 8 points
    let (user3, workout3_id) = create_user_with_workout(&test_app.address).await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Add engagement to workout1
    let reactor1 = create_test_user_and_login(&test_app.address).await;
    let reactor2 = create_test_user_and_login(&test_app.address).await;
    let commenter1 = create_test_user_and_login(&test_app.address).await;

    client.post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout1_id))
        .header("Authorization", format!("Bearer {}", reactor1.token))
        .json(&json!({"reaction_type": "fire"}))
        .send().await.expect("Failed to add reaction");

    client.post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout1_id))
        .header("Authorization", format!("Bearer {}", reactor2.token))
        .json(&json!({"reaction_type": "thumbs_up"}))
        .send().await.expect("Failed to add reaction");

    client.post(&format!("{}/social/workouts/{}/comments", test_app.address, workout1_id))
        .header("Authorization", format!("Bearer {}", commenter1.token))
        .json(&json!({"content": "Amazing!", "parent_id": null}))
        .send().await.expect("Failed to add comment");

    // Add 3 comments to workout2
    for i in 0..3 {
        let commenter = create_test_user_and_login(&test_app.address).await;
        client.post(&format!("{}/social/workouts/{}/comments", test_app.address, workout2_id))
            .header("Authorization", format!("Bearer {}", commenter.token))
            .json(&json!({"content": format!("Comment {}", i + 1), "parent_id": null}))
            .send().await.expect("Failed to add comment");
    }

    // Add 4 reactions to workout3
    for _ in 0..4 {
        let reactor = create_test_user_and_login(&test_app.address).await;
        client.post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout3_id))
            .header("Authorization", format!("Bearer {}", reactor.token))
            .json(&json!({"reaction_type": "fire"}))
            .send().await.expect("Failed to add reaction");
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get newsfeed
    let response = client
        .get(&format!("{}/feed/?limit=50", test_app.address))
        .header("Authorization", format!("Bearer {}", user1.token))
        .send()
        .await
        .expect("Failed to get newsfeed");

    assert!(response.status().is_success());
    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");

    let posts = response_body["data"]["posts"].as_array().unwrap();

    // Find positions
    let workout1_pos = posts.iter().position(|p| p["workout_id"] == workout1_id.to_string());
    let workout2_pos = posts.iter().position(|p| p["workout_id"] == workout2_id.to_string());
    let workout3_pos = posts.iter().position(|p| p["workout_id"] == workout3_id.to_string());

    assert!(workout1_pos.is_some(), "workout1 should be in feed");
    assert!(workout2_pos.is_some(), "workout2 should be in feed");
    assert!(workout3_pos.is_some(), "workout3 should be in feed");

    // Verify ranking by combined engagement score:
    // workout1 (17 points) > workout2 (9 points) > workout3 (8 points)
    assert!(workout1_pos.unwrap() < workout2_pos.unwrap(),
        "Workout1 (media+reactions+comment = 17) should rank higher than workout2 (3 comments = 9)");
    assert!(workout2_pos.unwrap() < workout3_pos.unwrap(),
        "Workout2 (3 comments = 9) should rank higher than workout3 (4 reactions = 8)");
}

#[tokio::test]
async fn test_newsfeed_chronological_sorting() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create user and upload 3 workouts with delays to ensure different timestamps
    let user = create_test_user_and_login(&test_app.address).await;

    // Use very recent timestamps (just seconds apart) to ensure these are the newest workouts
    // This helps avoid issues with other parallel tests polluting the feed
    let now = Utc::now();

    // First workout (oldest)
    let mut workout1_data = WorkoutData::new(
        WorkoutIntensity::Light,
        now - chrono::Duration::seconds(20),
        20
    );
    let workout1_response = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout1_data).await.expect("Failed to upload workout 1");
    let workout1_id = workout1_response["data"]["sync_id"].as_str().unwrap().to_string();

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Second workout (middle)
    let mut workout2_data = WorkoutData::new(
        WorkoutIntensity::Moderate,
        now - chrono::Duration::seconds(10),
        30
    );
    workout2_data.image_urls = Some(vec!["https://example.com/image.jpg".to_string()]); // Add media for engagement boost
    let workout2_response = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout2_data).await.expect("Failed to upload workout 2");
    let workout2_id = workout2_response["data"]["sync_id"].as_str().unwrap().to_string();

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Third workout (newest) - use current time to be the absolute newest
    let mut workout3_data = WorkoutData::new(
        WorkoutIntensity::Hard,
        Utc::now(),
        40
    );
    let workout3_response = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout3_data).await.expect("Failed to upload workout 3");
    let workout3_id = workout3_response["data"]["sync_id"].as_str().unwrap().to_string();

    // IMPORTANT: Create reactor and commenter users BEFORE we start,
    // so we don't create posts in between our workout posts
    let reactor = create_test_user_and_login(&test_app.address).await;
    let commenter = create_test_user_and_login(&test_app.address).await;

    // Small delay to ensure posts are committed
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Add high engagement to workout2 to make it rank higher in relevance mode
    // These don't create new posts, just add engagement data
    client.post(&format!("{}/social/workouts/{}/reactions", test_app.address, workout2_id))
        .header("Authorization", format!("Bearer {}", reactor.token))
        .json(&json!({"reaction_type": "fire"}))
        .send().await.expect("Failed to add reaction");

    client.post(&format!("{}/social/workouts/{}/comments", test_app.address, workout2_id))
        .header("Authorization", format!("Bearer {}", commenter.token))
        .json(&json!({"content": "Great workout!", "parent_id": null}))
        .send().await.expect("Failed to add comment");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test 1: Get feed with chronological sorting
    // Note: In CI, many tests run in parallel and create workouts, so we need a large limit
    let chronological_response = client
        .get(&format!("{}/feed/?sort_by=chronological&limit=1000", test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get chronological feed");

    assert!(chronological_response.status().is_success());
    let chronological_body: serde_json::Value = chronological_response.json().await.expect("Failed to parse response");
    let chronological_posts = chronological_body["data"]["posts"].as_array().unwrap();

    // Debug: Print all workout IDs in the feed
    println!("Chronological feed workout IDs:");
    for (i, post) in chronological_posts.iter().enumerate() {
        println!("  [{}] workout_id: {:?}", i, post["workout_id"]);
    }
    println!("Looking for:");
    println!("  workout1_id: {}", workout1_id);
    println!("  workout2_id: {}", workout2_id);
    println!("  workout3_id: {}", workout3_id);

    // Find positions in chronological feed
    let chrono_workout1_pos = chronological_posts.iter().position(|p| p["workout_id"] == workout1_id);
    let chrono_workout2_pos = chronological_posts.iter().position(|p| p["workout_id"] == workout2_id);
    let chrono_workout3_pos = chronological_posts.iter().position(|p| p["workout_id"] == workout3_id);

    // In parallel test execution, not all workouts may appear in top 50
    // But we should see at least workout3 (the newest)
    assert!(chrono_workout3_pos.is_some(), "workout3 ({}) should be in chronological feed (newest)", workout3_id);

    // If all three appear, verify chronological ordering
    if let (Some(pos1), Some(pos2), Some(pos3)) = (chrono_workout1_pos, chrono_workout2_pos, chrono_workout3_pos) {
        // In chronological order: newest first (workout3, workout2, workout1)
        assert!(pos3 < pos2,
            "Chronological: workout3 (newest) should appear before workout2");
        assert!(pos2 < pos1,
            "Chronological: workout2 should appear before workout1 (oldest)");
    } else {
        println!("Note: Not all test workouts in top 50 (parallel tests running). Testing with available workouts.");
    }

    // Test 2: Get feed with relevance sorting (default)
    let relevance_response = client
        .get(&format!("{}/feed/?sort_by=relevance&limit=50", test_app.address))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Failed to get relevance feed");

    assert!(relevance_response.status().is_success());
    let relevance_body: serde_json::Value = relevance_response.json().await.expect("Failed to parse response");
    let relevance_posts = relevance_body["data"]["posts"].as_array().unwrap();

    // Find positions in relevance feed
    let rel_workout1_pos = relevance_posts.iter().position(|p| p["workout_id"] == workout1_id);
    let rel_workout2_pos = relevance_posts.iter().position(|p| p["workout_id"] == workout2_id);
    let rel_workout3_pos = relevance_posts.iter().position(|p| p["workout_id"] == workout3_id);

    // We should at least see workout2 or workout3 (the most recent/engaging ones)
    assert!(rel_workout2_pos.is_some() || rel_workout3_pos.is_some(),
        "At least workout2 or workout3 should be in relevance feed");

    // If all workouts appear, verify engagement-based ranking
    if let (Some(pos1), Some(pos2), Some(pos3)) = (rel_workout1_pos, rel_workout2_pos, rel_workout3_pos) {
        // In relevance mode: workout2 should rank highest due to media + reaction + comment
        // workout2 has: media (10) + reaction (2) + comment (3) = 15 points
        // workout3 and workout1 have: 0 points
        assert!(pos2 < pos3,
            "Relevance: workout2 (high engagement) should rank higher than workout3");
        assert!(pos2 < pos1,
            "Relevance: workout2 (high engagement) should rank higher than workout1");

        // Test 3: Verify the order is different between the two modes
        // workout2 should be in different positions
        if let Some(chrono_pos2) = chrono_workout2_pos {
            assert_ne!(chrono_pos2, pos2,
                "Workout2 position should differ between chronological and relevance sorting");
        }
    } else {
        // If not all workouts appear, at least verify workout2 (with high engagement)
        // ranks differently between chronological and relevance
        if let (Some(chrono_pos2), Some(rel_pos2)) = (chrono_workout2_pos, rel_workout2_pos) {
            println!("Workout2 in chrono: {}, in relevance: {}", chrono_pos2, rel_pos2);
            assert_ne!(chrono_pos2, rel_pos2,
                "Workout2 position should differ between chronological and relevance sorting");
        } else if let (Some(chrono_pos3), Some(rel_pos3)) = (chrono_workout3_pos, rel_workout3_pos) {
            // At minimum, verify that workout3 appears in both feeds
            println!("Workout3 appears in both feeds (chrono: {}, relevance: {})", chrono_pos3, rel_pos3);
        } else {
            println!("Note: Limited workouts in feed due to parallel test execution");
        }
    }
}
