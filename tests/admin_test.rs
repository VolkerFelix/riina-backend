//! Consolidated admin functionality tests
//! 
//! This test suite covers all admin operations including:
//! - Authentication and authorization
//! - User management (CRUD operations)
//! - Team management (CRUD operations)  
//! - League management (CRUD operations)
//! - Error handling and edge cases

use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, make_authenticated_request, delete_test_user};
use common::admin_helpers::{create_admin_user_and_login, create_teams_for_test};

// ============================================================================
// AUTHENTICATION & AUTHORIZATION TESTS
// ============================================================================

#[tokio::test]
async fn admin_routes_require_authentication() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let test_routes = vec![
        ("GET", "/admin/users"),
        ("GET", "/admin/teams"),
        ("GET", "/admin/leagues"),
        ("GET", "/admin/users/without-team"),
    ];

    for (method, route) in test_routes {
        let response = client
            .request(method.parse().unwrap(), &format!("{}{}", test_app.address, route))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(
            401,
            response.status().as_u16(),
            "Route {} should require authentication",
            route
        );
    }
}

#[tokio::test]
async fn admin_routes_with_invalid_token_return_401() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let invalid_token = "invalid.jwt.token";

    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users", test_app.address),
        invalid_token,
        None,
    ).await;

    assert_eq!(401, response.status().as_u16());
}

#[tokio::test]
async fn admin_routes_return_proper_error_formats() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    // Try to get non-existent team
    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/teams/{}", test_app.address, Uuid::new_v4()),
        &admin.token,
        None,
    ).await;

    assert_eq!(404, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["error"].is_string());
}

// ============================================================================
// USER MANAGEMENT TESTS
// ============================================================================

#[tokio::test]
async fn admin_get_users_returns_paginated_results() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users", test_app.address),
        &admin.token,
        None,
    ).await;

    assert_eq!(200, response.status().as_u16());
    
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["data"].is_array());
    assert!(body["pagination"].is_object());
}

#[tokio::test]
async fn admin_get_users_with_search_filters_results() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users?search=admin&limit=5", test_app.address),
        &admin.token,
        None,
    ).await;

    assert_eq!(200, response.status().as_u16());
    
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["data"].is_array());
    let pagination = &body["pagination"];
    assert_eq!(5, pagination["limit"].as_i64().unwrap_or(0));
}

#[tokio::test]
async fn admin_get_user_by_id_returns_user_details() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users/{}", test_app.address, admin.user_id),
        &admin.token,
        None,
    ).await;

    assert_eq!(200, response.status().as_u16());
    
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(admin.user_id.to_string(), body["data"]["id"].as_str().unwrap());
}

#[tokio::test]
async fn admin_update_user_status_changes_user_status() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    let update_request = json!({
        "status": "inactive"
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::PATCH,
        &format!("{}/admin/users/{}/status", test_app.address, admin.user_id),
        &admin.token,
        Some(update_request),
    ).await;

    assert_eq!(200, response.status().as_u16());
    
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!("inactive", body["data"]["status"].as_str().unwrap());
}

#[tokio::test]
async fn admin_get_users_without_team_returns_filtered_users() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/users/without-team", test_app.address),
        &admin.token,
        None,
    ).await;

    assert_eq!(200, response.status().as_u16());
    
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["data"].is_array());
}

// ============================================================================
// TEAM MANAGEMENT TESTS
// ============================================================================

#[tokio::test]
async fn admin_create_team_succeeds_with_valid_data() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    let team_request = json!({
        "name": format!("Test Dragons {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#FF0000",
        "owner_id": admin.user_id
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &admin.token,
        Some(team_request),
    ).await;

    assert_eq!(201, response.status().as_u16());
    
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["data"]["name"].as_str().unwrap().starts_with("Test Dragons"));
    assert_eq!("#FF0000", body["data"]["color"].as_str().unwrap());
}

#[tokio::test]
async fn admin_get_teams_returns_paginated_results() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    // Create a test team first
    let team_request = json!({
        "name": format!("Test Team {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#00FF00",
        "owner_id": admin.user_id
    });

    make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &admin.token,
        Some(team_request),
    ).await;

    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/teams", test_app.address),
        &admin.token,
        None,
    ).await;

    assert_eq!(200, response.status().as_u16());
    
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["data"].is_array());
    assert!(body["pagination"].is_object());
}

#[tokio::test]
async fn admin_get_team_by_id_returns_team_details() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    // Create a test team first
    let team_request = json!({
        "name": format!("Phoenix Warriors {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#FFA500",
        "owner_id": admin.user_id
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &admin.token,
        Some(team_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.expect("Failed to parse create response");
    let team_id = create_body["data"]["id"].as_str().unwrap();

    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/teams/{}", test_app.address, team_id),
        &admin.token,
        None,
    ).await;

    assert_eq!(200, response.status().as_u16());
    
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(team_id, body["data"]["id"].as_str().unwrap());
    assert!(body["data"]["name"].as_str().unwrap().starts_with("Phoenix Warriors"));
}

#[tokio::test]
async fn admin_update_team_modifies_team_data() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    // Create a test team first
    let team_request = json!({
        "name": format!("Original Team {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#0000FF",
        "owner_id": admin.user_id
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &admin.token,
        Some(team_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.expect("Failed to parse create response");
    let team_id = create_body["data"]["id"].as_str().unwrap();

    let update_request = json!({
        "name": format!("Updated Team {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#FF00FF",
        "owner_id": admin.user_id
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::PATCH,
        &format!("{}/admin/teams/{}", test_app.address, team_id),
        &admin.token,
        Some(update_request),
    ).await;

    assert_eq!(200, response.status().as_u16());
    
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["data"]["name"].as_str().unwrap().starts_with("Updated Team"));
    assert_eq!("#FF00FF", body["data"]["color"].as_str().unwrap());
}

#[tokio::test]
async fn admin_update_team_owner_changes_ownership() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    // Create a second user to be the new owner
    let new_owner = create_test_user_and_login(&test_app.address).await;

    // Create a team with admin as owner
    let team_request = json!({
        "name": format!("Ownership Test Team {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#FF5733",
        "owner_id": admin.user_id
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &admin.token,
        Some(team_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.expect("Failed to parse create response");
    let team_id = create_body["data"]["id"].as_str().unwrap();

    // Add new owner as a team member first
    let add_member_request = json!({
        "user_id": new_owner.user_id,
        "role": "member"
    });

    let add_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams/{}/members", test_app.address, team_id),
        &admin.token,
        Some(add_member_request),
    ).await;

    assert_eq!(201, add_response.status().as_u16());

    // Update team to change owner
    let update_request = json!({
        "name": format!("Updated Owner Team {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#FF5733",
        "owner_id": new_owner.user_id
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::PATCH,
        &format!("{}/admin/teams/{}", test_app.address, team_id),
        &admin.token,
        Some(update_request),
    ).await;

    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(new_owner.user_id.to_string(), body["data"]["owner_id"].as_str().unwrap());

    // Verify team members have correct roles
    let members_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/teams/{}/members", test_app.address, team_id),
        &admin.token,
        None,
    ).await;

    assert_eq!(200, members_response.status().as_u16());

    let members_body: serde_json::Value = members_response.json().await.expect("Failed to parse members response");
    let members = members_body["data"].as_array().unwrap();

    // Find old and new owner in members list
    let new_owner_member = members.iter().find(|m| m["user_id"].as_str().unwrap() == new_owner.user_id.to_string());
    let old_owner_member = members.iter().find(|m| m["user_id"].as_str().unwrap() == admin.user_id.to_string());

    assert!(new_owner_member.is_some());
    assert_eq!("owner", new_owner_member.unwrap()["role"].as_str().unwrap());

    assert!(old_owner_member.is_some());
    assert_eq!("admin", old_owner_member.unwrap()["role"].as_str().unwrap());

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, new_owner.user_id).await;
}

#[tokio::test]
async fn admin_update_team_owner_fails_if_new_owner_not_member() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    // Create a second user who is NOT a team member
    let non_member = create_test_user_and_login(&test_app.address).await;

    // Create a team with admin as owner
    let team_request = json!({
        "name": format!("Test Team {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#0099FF",
        "owner_id": admin.user_id
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &admin.token,
        Some(team_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.expect("Failed to parse create response");
    let team_id = create_body["data"]["id"].as_str().unwrap();

    // Try to update team owner to someone who is not a member
    let update_request = json!({
        "name": "Updated Team",
        "color": "#0099FF",
        "owner_id": non_member.user_id
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::PATCH,
        &format!("{}/admin/teams/{}", test_app.address, team_id),
        &admin.token,
        Some(update_request),
    ).await;

    assert_eq!(400, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["error"].as_str().unwrap().contains("active team member"));

    // Cleanup
    delete_test_user(&test_app.address, &admin.token, non_member.user_id).await;
}

#[tokio::test]
async fn admin_delete_team_removes_team() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    // Create a test team first
    let team_request = json!({
        "name": format!("Doomed Team {}", &Uuid::new_v4().to_string()[..8]),
        "color": "#666666",
        "owner_id": admin.user_id
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/teams", test_app.address),
        &admin.token,
        Some(team_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.expect("Failed to parse create response");
    let team_id = create_body["data"]["id"].as_str().unwrap();

    let response = make_authenticated_request(
        &client,
        reqwest::Method::DELETE,
        &format!("{}/admin/teams/{}", test_app.address, team_id),
        &admin.token,
        None,
    ).await;

    assert_eq!(200, response.status().as_u16());

    // Verify team is deleted by trying to get it
    let get_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/teams/{}", test_app.address, team_id),
        &admin.token,
        None,
    ).await;

    assert_eq!(404, get_response.status().as_u16());
}

// ============================================================================
// LEAGUE MANAGEMENT TESTS
// ============================================================================

#[tokio::test]
async fn admin_get_leagues_returns_list_of_leagues() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/leagues", test_app.address),
        &admin.token,
        None,
    ).await;

    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["success"].as_bool().unwrap());
    assert!(body["data"].is_array());
}

#[tokio::test]
async fn admin_create_league_succeeds_with_valid_data() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    let league_request = json!({
        "name": format!("Test League {}", &Uuid::new_v4().to_string()[..8]),
        "description": "A test league for integration testing",
        "max_teams": 16
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", test_app.address),
        &admin.token,
        Some(league_request.clone()),
    ).await;

    assert_eq!(201, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["success"].as_bool().unwrap());
    assert!(body["data"]["id"].is_string());
    assert_eq!(body["data"]["name"].as_str().unwrap(), league_request["name"].as_str().unwrap());
    assert_eq!(body["data"]["max_teams"].as_i64().unwrap(), 16);
}

#[tokio::test]
async fn admin_create_league_with_invalid_max_teams_fails() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    let league_request = json!({
        "name": format!("Invalid League {}", &Uuid::new_v4().to_string()[..8]),
        "description": "A league with invalid max teams",
        "max_teams": 0
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", test_app.address),
        &admin.token,
        Some(league_request),
    ).await;

    assert_eq!(400, response.status().as_u16());
}

#[tokio::test]
async fn admin_get_league_by_id_returns_league_details() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    // Create a league first
    let league_request = json!({
        "name": format!("League for ID Test {}", &Uuid::new_v4().to_string()[..8]),
        "description": "Test league",
        "max_teams": 12
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", test_app.address),
        &admin.token,
        Some(league_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.unwrap();
    let league_id = create_body["data"]["id"].as_str().unwrap();

    let response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/leagues/{}", test_app.address, league_id),
        &admin.token,
        None,
    ).await;

    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["success"].as_bool().unwrap());
    assert_eq!(body["data"]["id"].as_str().unwrap(), league_id);
}

#[tokio::test]
async fn admin_update_league_modifies_league_data() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    // Create a league first
    let league_request = json!({
        "name": format!("Original League {}", &Uuid::new_v4().to_string()[..8]),
        "description": "Original description",
        "max_teams": 8
    });

    let create_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", test_app.address),
        &admin.token,
        Some(league_request),
    ).await;

    let create_body: serde_json::Value = create_response.json().await.unwrap();
    let league_id = create_body["data"]["id"].as_str().unwrap();

    // Update the league
    let update_request = json!({
        "name": format!("Updated League {}", &Uuid::new_v4().to_string()[..8])
    });

    let response = make_authenticated_request(
        &client,
        reqwest::Method::PATCH,
        &format!("{}/admin/leagues/{}", test_app.address, league_id),
        &admin.token,
        Some(update_request.clone()),
    ).await;

    assert_eq!(200, response.status().as_u16());

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    
    assert!(body["success"].as_bool().unwrap());
    assert_eq!(body["data"]["name"].as_str().unwrap(), update_request["name"].as_str().unwrap());
}

#[tokio::test]
async fn admin_assign_teams_to_league_works() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let admin = create_admin_user_and_login(&test_app.address, &test_app.db_pool).await;

    // Create a league
    let league_request = json!({
        "name": format!("Team Assignment League {}", &Uuid::new_v4().to_string()[..8]),
        "description": "Testing team assignment",
        "max_teams": 4
    });

    let league_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", test_app.address),
        &admin.token,
        Some(league_request),
    ).await;

    assert_eq!(201, league_response.status().as_u16());
    let league_body: serde_json::Value = league_response.json().await.expect("Failed to parse league response");
    let league_id = league_body["data"]["id"].as_str().expect("League ID not found");

    // Create teams
    let team_ids = create_teams_for_test(&test_app.address, &admin.token, 2).await;

    // Assign teams to league
    for team_id in &team_ids {
        let assign_request = json!({
            "team_id": team_id
        });

        let response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/leagues/{}/teams", test_app.address, league_id),
            &admin.token,
            Some(assign_request),
        ).await;

        assert_eq!(201, response.status().as_u16());
    }
}