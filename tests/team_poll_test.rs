use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use std::time::Duration;

mod common;
use common::utils::{spawn_app, create_test_user_and_login};
use common::admin_helpers::{create_admin_user_and_login, create_league, create_team, TeamConfig};

#[tokio::test]
async fn test_create_poll_success() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create team owner
    let owner = create_test_user_and_login(&test_app.address).await;

    // Create admin for league creation
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create league
    let league_id = create_league(
        &test_app.address,
        &admin.token,
        8  // max_teams
    ).await;

    // Create team
    let unique_suffix = &Uuid::new_v4().to_string()[..8];
    let team_id = create_team(
        &test_app.address,
        &admin.token,
        TeamConfig {
            name: Some(format!("Team A {}", unique_suffix)),
            color: Some("#FF0000".to_string()),
            description: Some("Team A".to_string()),
            owner_id: Some(owner.user_id),
        }
    ).await;

    // Create members
    let member1 = create_test_user_and_login(&test_app.address).await;
    let member2 = create_test_user_and_login(&test_app.address).await;
    let member3 = create_test_user_and_login(&test_app.address).await;

    // Add members to team
    let add_members = json!({
        "member_request": [
            {"username": member1.username, "role": "member"},
            {"username": member2.username, "role": "member"},
            {"username": member3.username, "role": "member"}
        ]
    });

    client
        .post(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&add_members)
        .send()
        .await
        .expect("Failed to add members");

    // Get member2's user_id
    let members_response = client
        .get(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", member1.token))
        .send()
        .await
        .expect("Failed to get team members");

    let members_body: serde_json::Value = members_response.json().await.unwrap();
    let members = members_body["data"]["members"].as_array().unwrap();
    let member2_data = members.iter().find(|m| m["username"] == member2.username).unwrap();
    let member2_id = member2_data["user_id"].as_str().unwrap();

    // Member1 creates a poll to remove member2
    let poll_data = json!({
        "target_user_id": member2_id,
        "poll_type": "member_removal"
    });

    let response = client
        .post(&format!("{}/league/teams/{}/polls", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", member1.token))
        .json(&poll_data)
        .send()
        .await
        .expect("Failed to create poll");

    assert_eq!(response.status().as_u16(), 201);

    let poll_body: serde_json::Value = response.json().await.unwrap();
    assert!(poll_body["success"].as_bool().unwrap());
    assert_eq!(poll_body["poll"]["target_user_id"], member2_id);
    assert_eq!(poll_body["poll"]["status"], "active");
    assert_eq!(poll_body["poll"]["total_eligible_voters"].as_u64().unwrap(), 4); // owner + member1 + member2 + member3 (all active members can vote)

    // Check notifications - owner should receive poll creation notification
    tokio::time::sleep(Duration::from_millis(100)).await;
    let notif_response = client.get(&format!("{}/social/notifications", test_app.address)).header("Authorization", format!("Bearer {}", owner.token)).send().await.unwrap();
    let notif_result: serde_json::Value = notif_response.json().await.unwrap();
    let notifications = notif_result["notifications"].as_array().unwrap();
    let poll_notif = notifications.iter().find(|n| n["notification_type"] == "team_poll_created");
    assert!(poll_notif.is_some(), "Owner should receive poll creation notification");

    // Member3 should also receive notification
    let notif_response = client.get(&format!("{}/social/notifications", test_app.address)).header("Authorization", format!("Bearer {}", member3.token)).send().await.unwrap();
    let notif_result: serde_json::Value = notif_response.json().await.unwrap();
    let notifications = notif_result["notifications"].as_array().unwrap();
    let poll_notif = notifications.iter().find(|n| n["notification_type"] == "team_poll_created");
    assert!(poll_notif.is_some(), "Member3 should receive poll creation notification");

    // Member2 (target) should ALSO receive poll creation notification (can vote on their own removal)
    let notif_response = client.get(&format!("{}/social/notifications", test_app.address)).header("Authorization", format!("Bearer {}", member2.token)).send().await.unwrap();
    let notif_result: serde_json::Value = notif_response.json().await.unwrap();
    let notifications = notif_result["notifications"].as_array().unwrap();
    let poll_notif = notifications.iter().find(|n| n["notification_type"] == "team_poll_created");
    assert!(poll_notif.is_some(), "Target should receive poll creation notification (can vote on their own removal)");
}

#[tokio::test]
async fn test_cannot_create_poll_for_owner() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create league
    let league_id = create_league(
        &test_app.address,
        &admin.token,
        8  // max_teams
    ).await;

    let unique_suffix = &Uuid::new_v4().to_string()[..8];
    let team_id = create_team(
        &test_app.address,
        &admin.token,
        TeamConfig {
            name: Some(format!("Team A {}", unique_suffix)),
            color: Some("#FF0000".to_string()),
            description: Some("Team A".to_string()),
            owner_id: Some(owner.user_id),
        }
    ).await;

    // Add member
    let member1 = create_test_user_and_login(&test_app.address).await;
    let add_members = json!({"member_request": [{"username": member1.username, "role": "member"}]});
    client
        .post(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&add_members)
        .send()
        .await
        .expect("Failed to add members");

    // Get owner's user_id
    let members_response = client
        .get(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", member1.token))
        .send()
        .await
        .unwrap();
    let members_body: serde_json::Value = members_response.json().await.unwrap();
    let members = members_body["data"]["members"].as_array().unwrap();
    let owner_data = members.iter().find(|m| m["role"] == "owner").unwrap();
    let owner_id = owner_data["user_id"].as_str().unwrap();

    // Try to create poll to remove owner (should fail)
    let poll_data = json!({"target_user_id": owner_id, "poll_type": "member_removal"});
    let response = client
        .post(&format!("{}/league/teams/{}/polls", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", member1.token))
        .json(&poll_data)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 403);
    let error_body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(error_body["success"].as_bool().unwrap(), false);
    assert!(error_body["message"].as_str().unwrap().contains("captain"));
}

#[tokio::test]
async fn test_can_create_poll_for_self() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create league
    let league_id = create_league(
        &test_app.address,
        &admin.token,
        8  // max_teams
    ).await;

    let unique_suffix = &Uuid::new_v4().to_string()[..8];
    let team_id = create_team(
        &test_app.address,
        &admin.token,
        TeamConfig {
            name: Some(format!("Team A {}", unique_suffix)),
            color: Some("#FF0000".to_string()),
            description: Some("Team A".to_string()),
            owner_id: Some(owner.user_id),
        }
    ).await;

    // Add members
    let member1 = create_test_user_and_login(&test_app.address).await;
    let member2 = create_test_user_and_login(&test_app.address).await;
    let add_members = json!({"member_request": [{"username": member1.username.clone(), "role": "member"}, {"username": member2.username, "role": "member"}]});
    client.post(&format!("{}/league/teams/{}/members", test_app.address, team_id)).header("Authorization", format!("Bearer {}", owner.token)).json(&add_members).send().await.unwrap();

    // Get member1's user_id
    let members_response = client.get(&format!("{}/league/teams/{}/members", test_app.address, team_id)).header("Authorization", format!("Bearer {}", member1.token)).send().await.unwrap();
    let members_body: serde_json::Value = members_response.json().await.unwrap();
    let members = members_body["data"]["members"].as_array().unwrap();
    let member1_data = members.iter().find(|m| m["username"] == member1.username).unwrap();
    let member1_id = member1_data["user_id"].as_str().unwrap();

    // Member1 creates a poll to remove themselves (should succeed)
    let poll_data = json!({"target_user_id": member1_id, "poll_type": "member_removal"});
    let response = client.post(&format!("{}/league/teams/{}/polls", test_app.address, team_id)).header("Authorization", format!("Bearer {}", member1.token)).json(&poll_data).send().await.unwrap();

    assert_eq!(response.status().as_u16(), 201);
    let poll_body: serde_json::Value = response.json().await.unwrap();
    assert!(poll_body["success"].as_bool().unwrap());
    assert_eq!(poll_body["poll"]["target_user_id"], member1_id);
}

#[tokio::test]
async fn test_cast_vote_and_check_result() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create league
    let league_id = create_league(
        &test_app.address,
        &admin.token,
        8  // max_teams
    ).await;

    let unique_suffix = &Uuid::new_v4().to_string()[..8];
    let team_id = create_team(
        &test_app.address,
        &admin.token,
        TeamConfig {
            name: Some(format!("Team A {}", unique_suffix)),
            color: Some("#FF0000".to_string()),
            description: Some("Team A".to_string()),
            owner_id: Some(owner.user_id),
        }
    ).await;

    // Create and add members
    let member1 = create_test_user_and_login(&test_app.address).await;
    let member2 = create_test_user_and_login(&test_app.address).await;
    let member3 = create_test_user_and_login(&test_app.address).await;
    let member4 = create_test_user_and_login(&test_app.address).await;
    let add_members = json!({"member_request": [
        {"username": member1.username.clone(), "role": "member"},
        {"username": member2.username.clone(), "role": "member"},
        {"username": member3.username.clone(), "role": "member"},
        {"username": member4.username.clone(), "role": "member"}
    ]});
    client.post(&format!("{}/league/teams/{}/members", test_app.address, team_id)).header("Authorization", format!("Bearer {}", owner.token)).json(&add_members).send().await.unwrap();

    // Get member4's ID
    let members_response = client.get(&format!("{}/league/teams/{}/members", test_app.address, team_id)).header("Authorization", format!("Bearer {}", member1.token)).send().await.unwrap();
    let members_body: serde_json::Value = members_response.json().await.unwrap();
    let members = members_body["data"]["members"].as_array().unwrap();
    let member4_data = members.iter().find(|m| m["username"] == member4.username).unwrap();
    let member4_id = member4_data["user_id"].as_str().unwrap();

    // Create poll
    let poll_data = json!({"target_user_id": member4_id, "poll_type": "member_removal"});
    let poll_response = client.post(&format!("{}/league/teams/{}/polls", test_app.address, team_id)).header("Authorization", format!("Bearer {}", member1.token)).json(&poll_data).send().await.unwrap();
    let poll_body: serde_json::Value = poll_response.json().await.unwrap();
    let poll_id = poll_body["poll"]["id"].as_str().unwrap();

    // Cast votes
    client.post(&format!("{}/league/teams/{}/polls/{}/vote", test_app.address, team_id, poll_id)).header("Authorization", format!("Bearer {}", member1.token)).json(&json!({"vote": "for"})).send().await.unwrap();
    client.post(&format!("{}/league/teams/{}/polls/{}/vote", test_app.address, team_id, poll_id)).header("Authorization", format!("Bearer {}", member2.token)).json(&json!({"vote": "for"})).send().await.unwrap();
    client.post(&format!("{}/league/teams/{}/polls/{}/vote", test_app.address, team_id, poll_id)).header("Authorization", format!("Bearer {}", member3.token)).json(&json!({"vote": "against"})).send().await.unwrap();

    // Get polls to check vote counts
    let polls_response = client.get(&format!("{}/league/teams/{}/polls", test_app.address, team_id)).header("Authorization", format!("Bearer {}", member1.token)).send().await.unwrap();
    let polls_body: serde_json::Value = polls_response.json().await.unwrap();
    let polls = polls_body["polls"].as_array().unwrap();
    let poll = &polls[0];

    assert_eq!(poll["votes_for"].as_u64().unwrap(), 2);
    assert_eq!(poll["votes_against"].as_u64().unwrap(), 1);
}

#[tokio::test]
async fn test_early_consensus_approval() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create league
    let league_id = create_league(
        &test_app.address,
        &admin.token,
        8  // max_teams
    ).await;

    let unique_suffix = &Uuid::new_v4().to_string()[..8];
    let team_id = create_team(
        &test_app.address,
        &admin.token,
        TeamConfig {
            name: Some(format!("Team A {}", unique_suffix)),
            color: Some("#FF0000".to_string()),
            description: Some("Team A".to_string()),
            owner_id: Some(owner.user_id),
        }
    ).await;

    // Create and add members
    let member1 = create_test_user_and_login(&test_app.address).await;
    let member2 = create_test_user_and_login(&test_app.address).await;
    let member3 = create_test_user_and_login(&test_app.address).await;
    let member4 = create_test_user_and_login(&test_app.address).await;
    let add_members = json!({"member_request": [
        {"username": member1.username.clone(), "role": "member"},
        {"username": member2.username.clone(), "role": "member"},
        {"username": member3.username.clone(), "role": "member"},
        {"username": member4.username.clone(), "role": "member"}
    ]});
    client.post(&format!("{}/league/teams/{}/members", test_app.address, team_id)).header("Authorization", format!("Bearer {}", owner.token)).json(&add_members).send().await.unwrap();

    // Get member4's ID
    let members_response = client.get(&format!("{}/league/teams/{}/members", test_app.address, team_id)).header("Authorization", format!("Bearer {}", member1.token)).send().await.unwrap();
    let members_body: serde_json::Value = members_response.json().await.unwrap();
    let members = members_body["data"]["members"].as_array().unwrap();
    let member4_data = members.iter().find(|m| m["username"] == member4.username).unwrap();
    let member4_id = member4_data["user_id"].as_str().unwrap();

    // Create poll (4 eligible voters: owner, member1, member2, member3)
    let poll_data = json!({"target_user_id": member4_id, "poll_type": "member_removal"});
    let poll_response = client.post(&format!("{}/league/teams/{}/polls", test_app.address, team_id)).header("Authorization", format!("Bearer {}", member1.token)).json(&poll_data).send().await.unwrap();
    let poll_body: serde_json::Value = poll_response.json().await.unwrap();
    let poll_id = poll_body["poll"]["id"].as_str().unwrap();

    // Cast 3 "for" votes to reach consensus (need 3 out of 4)
    client.post(&format!("{}/league/teams/{}/polls/{}/vote", test_app.address, team_id, poll_id)).header("Authorization", format!("Bearer {}", owner.token)).json(&json!({"vote": "for"})).send().await.unwrap();
    client.post(&format!("{}/league/teams/{}/polls/{}/vote", test_app.address, team_id, poll_id)).header("Authorization", format!("Bearer {}", member1.token)).json(&json!({"vote": "for"})).send().await.unwrap();
    client.post(&format!("{}/league/teams/{}/polls/{}/vote", test_app.address, team_id, poll_id)).header("Authorization", format!("Bearer {}", member2.token)).json(&json!({"vote": "for"})).send().await.unwrap();

    // Check poll status - should be completed
    let polls_response = client.get(&format!("{}/league/teams/{}/polls", test_app.address, team_id)).header("Authorization", format!("Bearer {}", member1.token)).send().await.unwrap();
    let polls_body: serde_json::Value = polls_response.json().await.unwrap();
    let polls = polls_body["polls"].as_array().unwrap();
    let poll = &polls[0];

    assert_eq!(poll["status"], "completed", "Poll should be completed after reaching consensus");
    assert_eq!(poll["result"], "approved", "Poll result should be approved");

    // Verify member4 was removed from team
    let members_response = client.get(&format!("{}/league/teams/{}/members", test_app.address, team_id)).header("Authorization", format!("Bearer {}", member1.token)).send().await.unwrap();
    let members_body: serde_json::Value = members_response.json().await.unwrap();
    let members = members_body["data"]["members"].as_array().unwrap();

    assert_eq!(members.len(), 4, "Should have 4 members (owner + 3 members)");
    assert!(members.iter().all(|m| m["username"] != member4.username), "Member4 should have been removed");

    // Check notifications - member4 should receive removal notification
    tokio::time::sleep(Duration::from_millis(100)).await;
    let notif_response = client.get(&format!("{}/social/notifications", test_app.address)).header("Authorization", format!("Bearer {}", member4.token)).send().await.unwrap();
    let notif_result: serde_json::Value = notif_response.json().await.unwrap();
    let notifications = notif_result["notifications"].as_array().unwrap();
    let removal_notif = notifications.iter().find(|n| n["notification_type"] == "removed_from_team");
    assert!(removal_notif.is_some(), "Removed member should receive removal notification");

    // Remaining members should receive completion notification
    let notif_response = client.get(&format!("{}/social/notifications", test_app.address)).header("Authorization", format!("Bearer {}", owner.token)).send().await.unwrap();
    let notif_result: serde_json::Value = notif_response.json().await.unwrap();
    let notifications = notif_result["notifications"].as_array().unwrap();
    let completion_notif = notifications.iter().find(|n| n["notification_type"] == "team_poll_completed");
    assert!(completion_notif.is_some(), "Team members should receive poll completion notification");

    let notif_response = client.get(&format!("{}/social/notifications", test_app.address)).header("Authorization", format!("Bearer {}", member1.token)).send().await.unwrap();
    let notif_result: serde_json::Value = notif_response.json().await.unwrap();
    let notifications = notif_result["notifications"].as_array().unwrap();
    let completion_notif = notifications.iter().find(|n| n["notification_type"] == "team_poll_completed");
    assert!(completion_notif.is_some(), "Poll creator should also receive completion notification");
}

#[tokio::test]
async fn test_cannot_vote_twice() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create league
    let league_id = create_league(
        &test_app.address,
        &admin.token,
        8  // max_teams
    ).await;

    let unique_suffix = &Uuid::new_v4().to_string()[..8];
    let team_id = create_team(
        &test_app.address,
        &admin.token,
        TeamConfig {
            name: Some(format!("Team A {}", unique_suffix)),
            color: Some("#FF0000".to_string()),
            description: Some("Team A".to_string()),
            owner_id: Some(owner.user_id),
        }
    ).await;

    // Add members
    let member1 = create_test_user_and_login(&test_app.address).await;
    let member2 = create_test_user_and_login(&test_app.address).await;
    let add_members = json!({"member_request": [{"username": member1.username.clone(), "role": "member"}, {"username": member2.username.clone(), "role": "member"}]});
    client.post(&format!("{}/league/teams/{}/members", test_app.address, team_id)).header("Authorization", format!("Bearer {}", owner.token)).json(&add_members).send().await.unwrap();

    // Get member2's ID
    let members_response = client.get(&format!("{}/league/teams/{}/members", test_app.address, team_id)).header("Authorization", format!("Bearer {}", member1.token)).send().await.unwrap();
    let members_body: serde_json::Value = members_response.json().await.unwrap();
    let members = members_body["data"]["members"].as_array().unwrap();
    let member2_data = members.iter().find(|m| m["username"] == member2.username).unwrap();
    let member2_id = member2_data["user_id"].as_str().unwrap();

    // Create poll
    let poll_data = json!({"target_user_id": member2_id, "poll_type": "member_removal"});
    let poll_response = client.post(&format!("{}/league/teams/{}/polls", test_app.address, team_id)).header("Authorization", format!("Bearer {}", member1.token)).json(&poll_data).send().await.unwrap();
    let poll_body: serde_json::Value = poll_response.json().await.unwrap();
    let poll_id = poll_body["poll"]["id"].as_str().unwrap();

    // Cast first vote
    let vote_response = client.post(&format!("{}/league/teams/{}/polls/{}/vote", test_app.address, team_id, poll_id)).header("Authorization", format!("Bearer {}", member1.token)).json(&json!({"vote": "for"})).send().await.unwrap();
    assert_eq!(vote_response.status().as_u16(), 200);

    // Try to cast second vote (should fail)
    let vote_response = client.post(&format!("{}/league/teams/{}/polls/{}/vote", test_app.address, team_id, poll_id)).header("Authorization", format!("Bearer {}", member1.token)).json(&json!({"vote": "for"})).send().await.unwrap();
    assert_eq!(vote_response.status().as_u16(), 409);

    let error_body: serde_json::Value = vote_response.json().await.unwrap();
    assert_eq!(error_body["success"].as_bool().unwrap(), false);
    assert!(error_body["message"].as_str().unwrap().contains("already voted"));
}

#[tokio::test]
async fn test_delete_poll_success() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create team owner
    let owner = create_test_user_and_login(&test_app.address).await;

    // Create admin for league creation
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create league
    let league_id = create_league(
        &test_app.address,
        &admin.token,
        8  // max_teams
    ).await;

    // Create team
    let unique_suffix = &Uuid::new_v4().to_string()[..8];
    let team_id = create_team(
        &test_app.address,
        &admin.token,
        TeamConfig {
            name: Some(format!("Team Delete {}", unique_suffix)),
            color: Some("#00FF00".to_string()),
            description: Some("Team for delete test".to_string()),
            owner_id: Some(owner.user_id),
        }
    ).await;

    // Create members
    let member1 = create_test_user_and_login(&test_app.address).await;
    let member2 = create_test_user_and_login(&test_app.address).await;

    // Add members to team
    let add_members = json!({
        "member_request": [
            {"username": member1.username, "role": "member"},
            {"username": member2.username, "role": "member"}
        ]
    });

    client.post(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&add_members)
        .send()
        .await
        .expect("Failed to add members");

    // Get member2's user_id
    let members_response = client.get(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    let members_body: serde_json::Value = members_response.json().await.unwrap();
    let members = members_body["data"]["members"].as_array().unwrap();
    let member2_data = members.iter().find(|m| m["username"] == member2.username).unwrap();
    let member2_id = member2_data["user_id"].as_str().unwrap();

    // Create a poll (owner creates poll to remove member2)
    let create_poll = json!({
        "target_user_id": member2_id,
        "poll_type": "member_removal"
    });

    let poll_response = client.post(&format!("{}/league/teams/{}/polls", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&create_poll)
        .send()
        .await
        .unwrap();

    assert_eq!(poll_response.status().as_u16(), 201);
    let poll_result: serde_json::Value = poll_response.json().await.unwrap();
    let poll_id = poll_result["poll"]["id"].as_str().unwrap();

    // Verify poll exists
    let get_polls_response = client.get(&format!("{}/league/teams/{}/polls", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();

    assert_eq!(get_polls_response.status().as_u16(), 200);
    let polls_result: serde_json::Value = get_polls_response.json().await.unwrap();
    assert_eq!(polls_result["polls"].as_array().unwrap().len(), 1);

    // Delete the poll (owner deletes)
    let delete_response = client.delete(&format!("{}/league/teams/{}/polls/{}", test_app.address, team_id, poll_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();

    assert_eq!(delete_response.status().as_u16(), 200);
    let delete_result: serde_json::Value = delete_response.json().await.unwrap();
    assert_eq!(delete_result["success"].as_bool().unwrap(), true);
    assert!(delete_result["message"].as_str().unwrap().contains("deleted successfully"));

    // Verify poll is deleted
    let get_polls_response = client.get(&format!("{}/league/teams/{}/polls", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();

    assert_eq!(get_polls_response.status().as_u16(), 200);
    let polls_result: serde_json::Value = get_polls_response.json().await.unwrap();
    assert_eq!(polls_result["polls"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_delete_poll_not_creator() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create team owner
    let owner = create_test_user_and_login(&test_app.address).await;

    // Create admin for league creation
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create league
    let league_id = create_league(
        &test_app.address,
        &admin.token,
        8  // max_teams
    ).await;

    // Create team
    let unique_suffix = &Uuid::new_v4().to_string()[..8];
    let team_id = create_team(
        &test_app.address,
        &admin.token,
        TeamConfig {
            name: Some(format!("Team NotCreator {}", unique_suffix)),
            color: Some("#0000FF".to_string()),
            description: Some("Team for not creator test".to_string()),
            owner_id: Some(owner.user_id),
        }
    ).await;

    // Create members
    let member1 = create_test_user_and_login(&test_app.address).await;
    let member2 = create_test_user_and_login(&test_app.address).await;

    // Add members to team
    let add_members = json!({
        "member_request": [
            {"username": member1.username, "role": "member"},
            {"username": member2.username, "role": "member"}
        ]
    });

    client.post(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&add_members)
        .send()
        .await
        .expect("Failed to add members");

    // Get member2's user_id
    let members_response = client.get(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    let members_body: serde_json::Value = members_response.json().await.unwrap();
    let members = members_body["data"]["members"].as_array().unwrap();
    let member2_data = members.iter().find(|m| m["username"] == member2.username).unwrap();
    let member2_id = member2_data["user_id"].as_str().unwrap();

    // Create a poll (owner creates poll to remove member2)
    let create_poll = json!({
        "target_user_id": member2_id,
        "poll_type": "member_removal"
    });

    let poll_response = client.post(&format!("{}/league/teams/{}/polls", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&create_poll)
        .send()
        .await
        .unwrap();

    assert_eq!(poll_response.status().as_u16(), 201);
    let poll_result: serde_json::Value = poll_response.json().await.unwrap();
    let poll_id = poll_result["poll"]["id"].as_str().unwrap();

    // Try to delete the poll as member1 (not the creator, should fail)
    let delete_response = client.delete(&format!("{}/league/teams/{}/polls/{}", test_app.address, team_id, poll_id))
        .header("Authorization", format!("Bearer {}", member1.token))
        .send()
        .await
        .unwrap();

    assert_eq!(delete_response.status().as_u16(), 403);
    let delete_result: serde_json::Value = delete_response.json().await.unwrap();
    assert_eq!(delete_result["success"].as_bool().unwrap(), false);
    assert!(delete_result["message"].as_str().unwrap().contains("Only the poll creator"));
}

#[tokio::test]
async fn test_delete_poll_after_completion() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create team owner
    let owner = create_test_user_and_login(&test_app.address).await;

    // Create admin for league creation
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create league
    let league_id = create_league(
        &test_app.address,
        &admin.token,
        8  // max_teams
    ).await;

    // Create team
    let unique_suffix = &Uuid::new_v4().to_string()[..8];
    let team_id = create_team(
        &test_app.address,
        &admin.token,
        TeamConfig {
            name: Some(format!("Team Completed {}", unique_suffix)),
            color: Some("#FFFF00".to_string()),
            description: Some("Team for completed poll test".to_string()),
            owner_id: Some(owner.user_id),
        }
    ).await;

    // Create members - need 4 members so we have 4 eligible voters after removing one
    let member1 = create_test_user_and_login(&test_app.address).await;
    let member2 = create_test_user_and_login(&test_app.address).await;
    let member3 = create_test_user_and_login(&test_app.address).await;
    let member4 = create_test_user_and_login(&test_app.address).await;

    // Add members to team
    let add_members = json!({
        "member_request": [
            {"username": member1.username, "role": "member"},
            {"username": member2.username, "role": "member"},
            {"username": member3.username, "role": "member"},
            {"username": member4.username, "role": "member"}
        ]
    });

    client.post(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&add_members)
        .send()
        .await
        .expect("Failed to add members");

    // Get member4's user_id (the one to be removed)
    let members_response = client.get(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    let members_body: serde_json::Value = members_response.json().await.unwrap();
    let members = members_body["data"]["members"].as_array().unwrap();
    let member4_data = members.iter().find(|m| m["username"] == member4.username).unwrap();
    let member4_id = member4_data["user_id"].as_str().unwrap();

    // Create a poll (owner creates poll to remove member4)
    // Eligible voters: owner, member1, member2, member3 (4 total)
    let create_poll = json!({
        "target_user_id": member4_id,
        "poll_type": "member_removal"
    });

    let poll_response = client.post(&format!("{}/league/teams/{}/polls", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&create_poll)
        .send()
        .await
        .unwrap();

    assert_eq!(poll_response.status().as_u16(), 201);
    let poll_result: serde_json::Value = poll_response.json().await.unwrap();
    let poll_id = poll_result["poll"]["id"].as_str().unwrap();

    // Cast votes to complete the poll (need 3 votes out of 4 eligible voters)
    // Owner votes for
    let vote_response = client.post(&format!("{}/league/teams/{}/polls/{}/vote", test_app.address, team_id, poll_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&json!({"vote": "for"}))
        .send()
        .await
        .unwrap();
    assert_eq!(vote_response.status().as_u16(), 200);

    // Member1 votes for
    let vote_response = client.post(&format!("{}/league/teams/{}/polls/{}/vote", test_app.address, team_id, poll_id))
        .header("Authorization", format!("Bearer {}", member1.token))
        .json(&json!({"vote": "for"}))
        .send()
        .await
        .unwrap();
    assert_eq!(vote_response.status().as_u16(), 200);

    // Member2 votes for (this should trigger approval and completion)
    let vote_response = client.post(&format!("{}/league/teams/{}/polls/{}/vote", test_app.address, team_id, poll_id))
        .header("Authorization", format!("Bearer {}", member2.token))
        .json(&json!({"vote": "for"}))
        .send()
        .await
        .unwrap();
    assert_eq!(vote_response.status().as_u16(), 200);

    // Try to delete the completed poll (should fail)
    let delete_response = client.delete(&format!("{}/league/teams/{}/polls/{}", test_app.address, team_id, poll_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();

    assert_eq!(delete_response.status().as_u16(), 400);
    let delete_result: serde_json::Value = delete_response.json().await.unwrap();
    assert_eq!(delete_result["success"].as_bool().unwrap(), false);
    assert!(delete_result["message"].as_str().unwrap().contains("Only active polls"));
}

#[tokio::test]
async fn test_removed_user_appears_in_leaderboard() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let owner = create_test_user_and_login(&test_app.address).await;
    let admin = create_admin_user_and_login(&test_app.address).await;

    // Create league
    let league_id = create_league(
        &test_app.address,
        &admin.token,
        8  // max_teams
    ).await;

    let unique_suffix = &Uuid::new_v4().to_string()[..8];
    let team_id = create_team(
        &test_app.address,
        &admin.token,
        TeamConfig {
            name: Some(format!("Team Leaderboard {}", unique_suffix)),
            color: Some("#FF00FF".to_string()),
            description: Some("Team for leaderboard test".to_string()),
            owner_id: Some(owner.user_id),
        }
    ).await;

    // Create and add members
    let member1 = create_test_user_and_login(&test_app.address).await;
    let member2 = create_test_user_and_login(&test_app.address).await;
    let member3 = create_test_user_and_login(&test_app.address).await;
    let member4 = create_test_user_and_login(&test_app.address).await;
    let add_members = json!({"member_request": [
        {"username": member1.username.clone(), "role": "member"},
        {"username": member2.username.clone(), "role": "member"},
        {"username": member3.username.clone(), "role": "member"},
        {"username": member4.username.clone(), "role": "member"}
    ]});
    client.post(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&add_members)
        .send()
        .await
        .unwrap();

    // Get member4's ID
    let members_response = client.get(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", member1.token))
        .send()
        .await
        .unwrap();
    let members_body: serde_json::Value = members_response.json().await.unwrap();
    let members = members_body["data"]["members"].as_array().unwrap();
    let member4_data = members.iter().find(|m| m["username"] == member4.username).unwrap();
    let member4_id = member4_data["user_id"].as_str().unwrap();

    // Create poll (5 eligible voters: owner, member1, member2, member3, member4)
    let poll_data = json!({"target_user_id": member4_id, "poll_type": "member_removal"});
    let poll_response = client.post(&format!("{}/league/teams/{}/polls", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", member1.token))
        .json(&poll_data)
        .send()
        .await
        .unwrap();
    let poll_body: serde_json::Value = poll_response.json().await.unwrap();
    let poll_id = poll_body["poll"]["id"].as_str().unwrap();

    // Cast 3 "for" votes to reach consensus (need 3 out of 5)
    client.post(&format!("{}/league/teams/{}/polls/{}/vote", test_app.address, team_id, poll_id))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&json!({"vote": "for"}))
        .send()
        .await
        .unwrap();
    client.post(&format!("{}/league/teams/{}/polls/{}/vote", test_app.address, team_id, poll_id))
        .header("Authorization", format!("Bearer {}", member1.token))
        .json(&json!({"vote": "for"}))
        .send()
        .await
        .unwrap();
    client.post(&format!("{}/league/teams/{}/polls/{}/vote", test_app.address, team_id, poll_id))
        .header("Authorization", format!("Bearer {}", member2.token))
        .json(&json!({"vote": "for"}))
        .send()
        .await
        .unwrap();

    // Verify member4 was removed from team
    let members_response = client.get(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", member1.token))
        .send()
        .await
        .unwrap();
    let members_body: serde_json::Value = members_response.json().await.unwrap();
    let members = members_body["data"]["members"].as_array().unwrap();
    assert!(members.iter().all(|m| m["username"] != member4.username), "Member4 should have been removed from team");

    // CRITICAL: Verify member4 is in player pool
    let pool_response = client.get(&format!("{}/league/player-pool", test_app.address))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(pool_response.status().as_u16(), 200);
    let pool_body: serde_json::Value = pool_response.json().await.unwrap();
    let pool_entries = pool_body["data"]["entries"].as_array().unwrap();
    let member4_in_pool = pool_entries.iter().any(|p| p["user_id"] == member4_id);
    assert!(member4_in_pool, "Member4 should be in player pool after being removed from team");

    // CRITICAL: Verify member4's profile still has a rank (proves they're in leaderboard)
    let profile_response = client.get(&format!("{}/profile/user", test_app.address))
        .header("Authorization", format!("Bearer {}", member4.token))
        .send()
        .await
        .unwrap();
    assert_eq!(profile_response.status().as_u16(), 200);
    let profile_body: serde_json::Value = profile_response.json().await.unwrap();
    let rank = profile_body["data"]["rank"].as_i64().unwrap();
    assert!(rank > 0 && rank < 999, "Member4 should have a valid rank (not 999) after being removed from team (proves they're in leaderboard). Got rank: {}", rank);
}
