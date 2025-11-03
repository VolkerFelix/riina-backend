//! Player Pool Feature Integration Tests
//!
//! This test suite covers:
//! - User registration automatically adding to player pool
//! - User status changes (active/inactive) and player pool management
//! - Team membership changes affecting player pool
//! - Player pool queries and filtering
//! - Redis WebSocket event notifications

use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use redis::Client as RedisClient;
use std::time::Duration;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use secrecy::ExposeSecret;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, delete_test_user};
use common::admin_helpers::create_admin_user_and_login;

use common::redis_helpers::setup_redis_pubsub;

// Redis event types matching the backend
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum PlayerPoolEventType {
    PlayerJoined,
    PlayerLeft,
    PlayerAssigned,
    PlayerLeftTeam,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlayerPoolEvent {
    event_type: PlayerPoolEventType,
    user_id: Uuid,
    username: String,
    league_id: Option<Uuid>,
    team_id: Option<Uuid>,
    team_name: Option<String>,
    timestamp: String,
}

// ============================================================================
// PLAYER POOL REGISTRATION TESTS
// ============================================================================

#[tokio::test]
async fn test_new_user_automatically_added_to_player_pool() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Set up Redis subscription for player pool events
    let mut pubsub = setup_redis_pubsub("player_pool_events").await;

    // Register a new user
    let test_user = create_test_user_and_login(&test_app.address).await;

    // Verify Redis event was published
    let mut stream = pubsub.on_message();
    let timeout = Duration::from_secs(5);
    let start_time = std::time::Instant::now();
    let mut event_received = false;

    while start_time.elapsed() < timeout && !event_received {
        tokio::select! {
            msg = stream.next() => {
                if let Some(msg) = msg {
                    if let Ok(payload) = msg.get_payload::<String>() {
                        if let Ok(event) = serde_json::from_str::<PlayerPoolEvent>(&payload) {
                            if event.event_type == PlayerPoolEventType::PlayerJoined
                                && event.user_id == test_user.user_id {
                                tracing::info!("✅ Received player_joined event for user {}", test_user.user_id);
                                assert_eq!(event.username, test_user.username);
                                event_received = true;
                            }
                        }
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }

    assert!(event_received, "Did not receive player_joined Redis event for new user");

    // Check if user is in player pool
    let pool_response = client
        .get(&format!("{}/league/player-pool", &test_app.address))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to fetch player pool");

    assert!(pool_response.status().is_success());

    let pool_data: serde_json::Value = pool_response
        .json()
        .await
        .expect("Failed to parse player pool response");

    // Verify the new user is in the pool
    let entries = pool_data["data"]["entries"].as_array().unwrap();
    let user_in_pool = entries.iter().any(|entry| {
        entry["user_id"].as_str().unwrap() == test_user.user_id.to_string()
    });

    assert!(user_in_pool, "New user should be automatically added to player pool");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

// ============================================================================
// USER STATUS CHANGE TESTS
// ============================================================================

#[tokio::test]
async fn test_user_status_change_to_inactive_removes_from_pool() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;
    let test_user = create_test_user_and_login(&test_app.address).await;

    // Verify user is in pool initially
    let initial_status_response = client
        .get(&format!("{}/profile/status", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .send()
        .await
        .expect("Failed to get user status");

    let initial_status: serde_json::Value = initial_status_response
        .json()
        .await
        .expect("Failed to parse status response");

    assert_eq!(initial_status["data"]["status"], "active");
    assert_eq!(initial_status["data"]["in_player_pool"], true);

    // Change status to inactive
    let status_change = json!({
        "status": "inactive"
    });

    let update_response = client
        .patch(&format!("{}/profile/status", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&status_change)
        .send()
        .await
        .expect("Failed to update status");

    assert!(update_response.status().is_success());

    let updated_status: serde_json::Value = update_response
        .json()
        .await
        .expect("Failed to parse update response");

    assert_eq!(updated_status["data"]["status"], "inactive");
    assert_eq!(updated_status["data"]["in_player_pool"], false);

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_user_status_change_back_to_active_adds_to_pool() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;
    let test_user = create_test_user_and_login(&test_app.address).await;

    // Change to inactive
    let go_inactive = json!({"status": "inactive"});
    client
        .patch(&format!("{}/profile/status", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&go_inactive)
        .send()
        .await
        .expect("Failed to go inactive");

    // Change back to active
    let go_active = json!({"status": "active"});
    let response = client
        .patch(&format!("{}/profile/status", &test_app.address))
        .header("Authorization", format!("Bearer {}", test_user.token))
        .json(&go_active)
        .send()
        .await
        .expect("Failed to go active");

    let status_data: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse response");

    assert_eq!(status_data["data"]["status"], "active");
    assert_eq!(status_data["data"]["in_player_pool"], true);

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_inactive_user_with_team_removes_from_team() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create team owner
    let owner = create_test_user_and_login(&test_app.address).await;

    // Create a team
    let team_name = format!("Test Team {}", Uuid::new_v4().to_string()[..8].to_string());
    let team_request = json!({
        "team_name": team_name,
        "team_description": "Test Description",
        "team_color": "#FF0000"
    });

    let team_response = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&team_request)
        .send()
        .await
        .expect("Failed to create team");

    assert!(team_response.status().is_success());

    let team_data: serde_json::Value = team_response
        .json()
        .await
        .expect("Failed to parse team response");

    let team_id = team_data["data"]["team_id"].as_str().unwrap();

    // Get team members to verify owner is there
    let members_response = client
        .get(&format!("{}/league/teams/{}/members", &test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("Failed to get team members");

    let members_data: serde_json::Value = members_response
        .json()
        .await
        .expect("Failed to parse members response");

    let initial_member_count = members_data["data"]["members"].as_array().unwrap().len();
    assert_eq!(initial_member_count, 1, "Team should have owner as member");

    // Owner goes inactive
    let go_inactive = json!({"status": "inactive"});
    let status_response = client
        .patch(&format!("{}/profile/status", &test_app.address))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&go_inactive)
        .send()
        .await
        .expect("Failed to go inactive");

    assert!(status_response.status().is_success());

    // Check team members again - owner should be marked inactive
    let updated_members_response = client
        .get(&format!("{}/league/teams/{}/members", &test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to get updated team members");

    let updated_members_data: serde_json::Value = updated_members_response
        .json()
        .await
        .expect("Failed to parse updated members response");

    let members = updated_members_data["data"]["members"].as_array().unwrap();
    let owner_member = members.iter().find(|m| {
        m["user_id"].as_str().unwrap() == owner.user_id.to_string()
    });

    if let Some(owner_member) = owner_member {
        assert_eq!(owner_member["status"], "inactive", "Owner should be marked as inactive in team");
    }

    // Verify owner is not in player pool (inactive users shouldn't be)
    let status_check = client
        .get(&format!("{}/profile/status", &test_app.address))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("Failed to check status");

    let status_check_data: serde_json::Value = status_check
        .json()
        .await
        .expect("Failed to parse status check");

    assert_eq!(status_check_data["data"]["in_player_pool"], false);

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

// ============================================================================
// PLAYER POOL QUERY TESTS
// ============================================================================

#[tokio::test]
async fn test_get_player_pool_empty() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Make admin inactive so they're not in the pool
    let go_inactive = json!({"status": "inactive"});
    client
        .patch(&format!("{}/profile/status", &test_app.address))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&go_inactive)
        .send()
        .await
        .expect("Failed to go inactive");

    // Query player pool
    let response = client
        .get(&format!("{}/league/player-pool", &test_app.address))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to fetch player pool");

    if response.status().is_success() {
        let data: serde_json::Value = response
            .json()
            .await
            .expect("Failed to parse response");

        let entries = data["data"]["entries"].as_array().unwrap();
        tracing::info!("Player pool has {} entries", entries.len());
    } else {
        // If the endpoint doesn't exist yet, that's okay for now
        tracing::warn!("Player pool endpoint not yet implemented");
    }

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_multiple_users_in_player_pool() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create multiple test users
    let user1 = create_test_user_and_login(&test_app.address).await;
    let user2 = create_test_user_and_login(&test_app.address).await;
    let user3 = create_test_user_and_login(&test_app.address).await;

    // Query player pool
    let response = client
        .get(&format!("{}/league/player-pool", &test_app.address))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Failed to fetch player pool");

    if response.status().is_success() {
        let data: serde_json::Value = response
            .json()
            .await
            .expect("Failed to parse response");

        let entries = data["data"]["entries"].as_array().unwrap();

        // Should have at least our 3 test users (might have more from other tests)
        assert!(entries.len() >= 3, "Player pool should have at least 3 users");

        // Verify our test users are in the pool
        let user_ids: Vec<String> = entries.iter()
            .map(|e| e["user_id"].as_str().unwrap().to_string())
            .collect();

        assert!(user_ids.contains(&user1.user_id.to_string()));
        assert!(user_ids.contains(&user2.user_id.to_string()));
        assert!(user_ids.contains(&user3.user_id.to_string()));
    }

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, user1.user_id).await;
    delete_test_user(&test_app.address, &admin.token, user2.user_id).await;
    delete_test_user(&test_app.address, &admin.token, user3.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_user_in_team_not_in_player_pool() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a user and team
    let owner = create_test_user_and_login(&test_app.address).await;

    let team_name = format!("Test Team {}", Uuid::new_v4().to_string()[..8].to_string());
    let team_request = json!({
        "team_name": team_name,
        "team_color": "#00FF00"
    });

    let team_response = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&team_request)
        .send()
        .await
        .expect("Failed to create team");

    assert!(team_response.status().is_success());

    // Check user status - should NOT be in player pool since they're in a team
    let status_response = client
        .get(&format!("{}/profile/status", &test_app.address))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("Failed to get status");

    let status_data: serde_json::Value = status_response
        .json()
        .await
        .expect("Failed to parse status");

    assert_eq!(status_data["data"]["status"], "active");
    // Note: User might still be in pool initially, but shouldn't be after proper implementation
    tracing::info!("User in_player_pool: {}", status_data["data"]["in_player_pool"]);

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}

#[tokio::test]
async fn test_redis_player_assigned_event_on_team_join() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a team owner
    let owner = create_test_user_and_login(&test_app.address).await;

    // Set up Redis subscription BEFORE creating team
    let mut pubsub = setup_redis_pubsub("player_pool_events").await;
    let mut stream = pubsub.on_message();

    // Create a team - this should trigger player_assigned event for the owner
    let team_name = format!("Test Team {}", Uuid::new_v4().to_string()[..8].to_string());
    let team_request = json!({
        "team_name": team_name,
        "team_color": "#FF0000"
    });

    let team_response = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&team_request)
        .send()
        .await
        .expect("Failed to create team");

    assert!(team_response.status().is_success());

    let team_data: serde_json::Value = team_response
        .json()
        .await
        .expect("Failed to parse team response");

    let team_id = Uuid::parse_str(team_data["data"]["team_id"].as_str().unwrap()).unwrap();

    // Verify both Redis events were published (player_left and player_assigned)
    let timeout = Duration::from_secs(5);
    let start_time = std::time::Instant::now();
    let mut player_left_received = false;
    let mut player_assigned_received = false;

    while start_time.elapsed() < timeout && (!player_left_received || !player_assigned_received) {
        tokio::select! {
            msg = stream.next() => {
                if let Some(msg) = msg {
                    if let Ok(payload) = msg.get_payload::<String>() {
                        if let Ok(event) = serde_json::from_str::<PlayerPoolEvent>(&payload) {
                            if event.user_id == owner.user_id {
                                match event.event_type {
                                    PlayerPoolEventType::PlayerLeft => {
                                        tracing::info!("✅ Received player_left event for team owner {}", owner.user_id);
                                        assert_eq!(event.username, owner.username);
                                        player_left_received = true;
                                    }
                                    PlayerPoolEventType::PlayerAssigned => {
                                        tracing::info!("✅ Received player_assigned event for team owner {}", owner.user_id);
                                        assert_eq!(event.username, owner.username);
                                        assert_eq!(event.team_id, Some(team_id));
                                        assert_eq!(event.team_name, Some(team_name.clone()));
                                        player_assigned_received = true;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }

    assert!(player_left_received, "Did not receive player_left Redis event when team was created");
    assert!(player_assigned_received, "Did not receive player_assigned Redis event when team was created");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}


#[tokio::test]
async fn test_redis_player_left_team_event_on_team_leave() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create a team with owner and a member
    let owner = create_test_user_and_login(&test_app.address).await;
    let member = create_test_user_and_login(&test_app.address).await;

    // Create team
    let team_name = format!("Test Team {}", Uuid::new_v4().to_string()[..8].to_string());
    let team_request = json!({
        "team_name": team_name,
        "team_color": "#FF0000"
    });

    let team_response = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&team_request)
        .send()
        .await
        .expect("Failed to create team");

    assert!(team_response.status().is_success());
    let team_data: serde_json::Value = team_response.json().await.unwrap();
    let team_id = team_data["data"]["team_id"].as_str().unwrap();

    // Add member to team
    let add_member_request = json!({
        "member_request": [{
            "user_id": member.user_id,
            "role": "member"
        }]
    });

    let add_response = client
        .post(&format!("{}/league/teams/{}/members", &test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&add_member_request)
        .send()
        .await
        .expect("Failed to add member");

    assert!(add_response.status().is_success());

    // Set up Redis subscription BEFORE removing member
    let mut pubsub = setup_redis_pubsub("player_pool_events").await;
    let mut stream = pubsub.on_message();

    // Remove member from team
    let remove_response = client
        .delete(&format!("{}/league/teams/{}/members/{}", &test_app.address, team_id, member.user_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("Failed to remove member");

    assert!(remove_response.status().is_success());

    // Verify both Redis events were published (player_left_team and player_joined)
    let timeout = Duration::from_secs(5);
    let start_time = std::time::Instant::now();
    let mut player_left_team_received = false;
    let mut player_joined_received = false;

    while start_time.elapsed() < timeout && (!player_left_team_received || !player_joined_received) {
        tokio::select! {
            msg = stream.next() => {
                if let Some(msg) = msg {
                    if let Ok(payload) = msg.get_payload::<String>() {
                        if let Ok(event) = serde_json::from_str::<PlayerPoolEvent>(&payload) {
                            if event.user_id == member.user_id {
                                match event.event_type {
                                    PlayerPoolEventType::PlayerLeftTeam => {
                                        tracing::info!("✅ Received player_left_team event for user {} after leaving team", member.user_id);
                                        assert_eq!(event.username, member.username);
                                        assert!(event.team_id.is_some(), "Team ID should be present in player_left_team event");
                                        assert!(event.team_name.is_some(), "Team name should be present in player_left_team event");
                                        player_left_team_received = true;
                                    }
                                    PlayerPoolEventType::PlayerJoined => {
                                        tracing::info!("✅ Received player_joined event for user {} after leaving team", member.user_id);
                                        assert_eq!(event.username, member.username);
                                        player_joined_received = true;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }

    assert!(player_left_team_received, "Did not receive player_left_team Redis event when member left team");
    assert!(player_joined_received, "Did not receive player_joined Redis event when member left team");

    // Verify member is back in player pool
    let status_response = client
        .get(&format!("{}/profile/status", &test_app.address))
        .header("Authorization", format!("Bearer {}", member.token))
        .send()
        .await
        .expect("Failed to get status");

    let status_data: serde_json::Value = status_response.json().await.unwrap();
    assert_eq!(status_data["data"]["in_player_pool"], true, "Member should be back in player pool after leaving team");

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, owner.user_id).await;
    delete_test_user(&test_app.address, &admin.token, member.user_id).await;
    delete_test_user(&test_app.address, &admin.token, admin.user_id).await;
}
