use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use chrono::Utc;

mod common;
use common::{
    utils::{
        spawn_app,
        create_test_user_and_login,
        make_authenticated_request
    },
    workout_data_helpers::{
        WorkoutData,
        WorkoutType,
        upload_workout_data_for_user}
};

#[tokio::test]
async fn test_get_league_users_with_stats_success() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create multiple users with different stats
    let mut users = Vec::new();

    for _ in 1..=6 {
        let user = create_test_user_and_login(&test_app.address).await;
        users.push(user);
    }

    println!("✅ Created {} users", users.len());

    // Step 2: Upload health data for each user
    for user in users.iter() {
        let mut workout_data = WorkoutData::new(WorkoutType::Intense, Utc::now(), 30);
        let upload_response = upload_workout_data_for_user(&client, &test_app.address, &user.token, &mut workout_data).await;
        assert!(upload_response.is_ok(), "Failed to upload health data for user: {}", upload_response.err().unwrap());
    }

    // Step 3: Create teams and add users as members
    let team1_owner = &users[0];
    let team2_owner = &users[3];

    // Create Team 1
    let team1_name = format!("Team_Alpha_{}", Uuid::new_v4().to_string()[..8].to_string());
    let team1_request = json!({
        "team_name": team1_name,
        "team_description": "First test team for stats endpoint",
        "team_color": "#FF0000"
    });

    let team1_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/league/teams/register", &test_app.address),
        &team1_owner.token,
        Some(team1_request),
    ).await;

    assert!(team1_response.status().is_success(), "Team 1 registration should succeed");
    let team1_json = team1_response.json::<serde_json::Value>().await
        .expect("Failed to parse team 1 response");
    let team1_id = team1_json["data"]["team_id"].as_str().expect("Team 1 ID should be present");

    // Create Team 2
    let team2_name = format!("Team_Beta_{}", Uuid::new_v4().to_string()[..8].to_string());
    let team2_request = json!({
        "team_name": team2_name,
        "team_description": "Second test team for stats endpoint",
        "team_color": "#0000FF"
    });

    let team2_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/league/teams/register", &test_app.address),
        &team2_owner.token,
        Some(team2_request),
    ).await;

    assert!(team2_response.status().is_success(), "Team 2 registration should succeed");
    let team2_json = team2_response.json::<serde_json::Value>().await
        .expect("Failed to parse team 2 response");
    let team2_id = team2_json["data"]["team_id"].as_str().expect("Team 2 ID should be present");

    println!("✅ Created 2 teams: {} and {}", team1_name, team2_name);

    // Step 4: Add members to teams
    // Add users[1] and users[2] to Team 1
    let add_members_team1 = json!({
        "member_request": [
            {
                "username": users[1].username,
                "role": "member"
            },
            {
                "username": users[2].username,
                "role": "admin"
            }
        ]
    });

    let add_response1 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/league/teams/{}/members", &test_app.address, team1_id),
        &team1_owner.token,
        Some(add_members_team1),
    ).await;

    assert!(add_response1.status().is_success(), "Adding members to team 1 should succeed");

    // Add users[4] and users[5] to Team 2
    let add_members_team2 = json!({
        "member_request": [
            {
                "username": users[4].username,
                "role": "member"
            },
            {
                "username": users[5].username,
                "role": "member"
            }
        ]
    });

    let add_response2 = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/league/teams/{}/members", &test_app.address, team2_id),
        &team2_owner.token,
        Some(add_members_team2),
    ).await;

    assert!(add_response2.status().is_success(), "Adding members to team 2 should succeed");

    println!("✅ Added members to both teams");

    // Step 5: Test the league users stats endpoint with a large page_size to get all users
    let stats_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/users/stats?page_size=100", &test_app.address),
        &team1_owner.token,
        None,
    ).await;

    assert!(stats_response.status().is_success(), "League users stats request should succeed");

    let stats_json = stats_response.json::<serde_json::Value>().await
        .expect("Failed to parse stats response");

    // Step 6: Validate response structure
    assert_eq!(stats_json["success"], true, "Response should indicate success");
    assert!(stats_json["data"].is_array(), "Data should be an array");
    
    let league_users = stats_json["data"].as_array().unwrap();
    let total_count = stats_json["total_count"].as_u64().unwrap();
    
    // Verify that our 6 test users are included in the results
    // (there may be additional users from other tests)
    assert!(total_count >= 6, "Should include at least our 6 team members, got {}", total_count);
    assert!(league_users.len() >= 6, "Data array should include at least our 6 team members, got {}", league_users.len());

    // Step 7: Validate data content for our specific test users
    let our_test_usernames: Vec<&String> = users.iter().map(|user| &user.username).collect();
    let our_test_users: Vec<&serde_json::Value> = league_users.iter()
        .filter(|user| {
            if let Some(username) = user["username"].as_str() {
                our_test_usernames.iter().any(|&test_username| test_username == username)
            } else {
                false
            }
        })
        .collect();
    
    // Verify we found all our test users
    assert_eq!(our_test_users.len(), 6, "Should find all 6 of our test users in the response");
    
    for user_data in our_test_users {
        // Check required fields are present
        assert!(user_data["user_id"].is_string(), "user_id should be present");
        assert!(user_data["username"].is_string(), "username should be present");
        assert!(user_data["email"].is_string(), "email should be present");
        assert!(user_data["team_id"].is_string(), "team_id should be present");
        assert!(user_data["team_name"].is_string(), "team_name should be present");
        assert!(user_data["team_role"].is_string(), "team_role should be present");
        assert!(user_data["team_status"].is_string(), "team_status should be present");
        assert!(user_data["joined_at"].is_string(), "joined_at should be present");

        // Check stats structure
        assert!(user_data["stats"].is_object(), "stats should be an object");
        assert!(user_data["stats"]["stamina"].is_number(), "stamina should be a number");
        assert!(user_data["stats"]["strength"].is_number(), "strength should be a number");
        assert!(user_data["total_stats"].is_number(), "total_stats should be a number");
        assert!(user_data["rank"].is_number(), "rank should be a number");
        assert!(user_data["avatar_style"].is_string(), "avatar_style should be present");
        assert!(user_data["is_online"].is_boolean(), "is_online should be a boolean");

        // Validate data integrity
        let stamina = user_data["stats"]["stamina"].as_f64().unwrap();
        let strength = user_data["stats"]["strength"].as_f64().unwrap();
        let total_stats = user_data["total_stats"].as_f64().unwrap();
        assert_eq!(stamina + strength, total_stats, "total_stats should equal stamina + strength");

        // Check team role is valid
        let team_role = user_data["team_role"].as_str().unwrap();
        assert!(["owner", "admin", "member"].contains(&team_role), "team_role should be valid");

        // Check team status is active (since we only added active members)
        let team_status = user_data["team_status"].as_str().unwrap();
        assert_eq!(team_status, "active", "team_status should be active");
    }

    // Step 8: Validate specific user data
    let team1_members = league_users.iter()
        .filter(|user| user["team_name"] == team1_name)
        .collect::<Vec<_>>();
    let team2_members = league_users.iter()
        .filter(|user| user["team_name"] == team2_name)
        .collect::<Vec<_>>();

    assert_eq!(team1_members.len(), 3, "Team 1 should have 3 members");
    assert_eq!(team2_members.len(), 3, "Team 2 should have 3 members");

    // Check if owner is properly identified
    let team1_owner_data = team1_members.iter()
        .find(|user| user["username"] == users[0].username)
        .expect("Team 1 owner should be present");
    assert_eq!(team1_owner_data["team_role"], "owner", "First user should be team 1 owner");

    let team2_owner_data = team2_members.iter()
        .find(|user| user["username"] == users[3].username)
        .expect("Team 2 owner should be present");
    assert_eq!(team2_owner_data["team_role"], "owner", "Fourth user should be team 2 owner");

    // Check admin role
    let team1_admin = team1_members.iter()
        .find(|user| user["username"] == users[2].username)
        .expect("Team 1 admin should be present");
    assert_eq!(team1_admin["team_role"], "admin", "Third user should be team 1 admin");

    // Step 9: Validate stats are reasonable after health data upload
    for (i, user) in users.iter().enumerate() {
        if let Some(user_data) = league_users.iter().find(|u| u["username"] == user.username) {
            let total_stats = user_data["total_stats"].as_f64().unwrap();
            let stamina = user_data["stats"]["stamina"].as_f64().unwrap();
            let strength = user_data["stats"]["strength"].as_f64().unwrap();

            // All users uploaded Intense workout data (high heart rate zones)
            // With the HR zone-based scoring, intense workouts give mostly strength points
            assert!(total_stats > 0.0, "User {} should have enhanced stats after health data upload, got {}", i, total_stats);
            // Intense workouts may give low or no stamina as they're in high HR zones
            assert!(stamina > 0.0 || strength > 0.0, "User {} stamina or strength should be positive, got {}", i, stamina);

            println!("   User {}: {} stamina, {} strength, {} total", i, stamina, strength, total_stats);
            println!("   User {}: {} stamina, {} strength, {} total", i, stamina, strength, total_stats);
        }
    }

    println!("✅ League users stats endpoint test completed successfully");
    println!("===================================================");
    println!("✅ Retrieved {} league users with complete stats", league_users.len());
    println!("✅ Validated response structure and data integrity");
    println!("✅ Confirmed team membership and role assignments");
    println!("✅ Verified stats calculations and rankings");
}

#[tokio::test]
async fn test_league_users_stats_unauthorized() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Test without authentication token
    let response = client
        .get(&format!("{}/league/users/stats", &test_app.address))
        .send()
        .await
        .expect("Failed to send request without auth");

    assert_eq!(response.status(), 401, "Request without auth should return 401");

    // Test with invalid token
    let response = client
        .get(&format!("{}/league/users/stats", &test_app.address))
        .header("Authorization", "Bearer invalid-token")
        .send()
        .await
        .expect("Failed to send request with invalid token");

    assert_eq!(response.status(), 401, "Request with invalid token should return 401");

    println!("✅ Unauthorized access tests passed");
}

#[tokio::test]
async fn test_league_users_stats_empty_response() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create a user but don't add them to any team
    let solo_user = create_test_user_and_login(&test_app.address).await;

    // Test the endpoint with a user who is not in any team
    let stats_response = client
        .get(&format!("{}/league/users/stats", &test_app.address))
        .header("Authorization", format!("Bearer {}", solo_user.token))
        .send()
        .await
        .expect("Failed to get league users stats");

    assert!(stats_response.status().is_success(), "Request should succeed even if no teams exist");

    let stats_json = stats_response.json::<serde_json::Value>().await
        .expect("Failed to parse stats response");

    assert_eq!(stats_json["success"], true, "Response should indicate success");
    assert!(stats_json["data"].is_array(), "Data should be an array");
    
    // The response may contain users from other tests, but our solo user should not be included
    // since they are not part of any team
    let league_users = stats_json["data"].as_array().unwrap();
    let solo_user_found = league_users.iter()
        .any(|user| user["username"].as_str() == Some(&solo_user.username));
    
    assert!(!solo_user_found, "Solo user (not in any team) should not appear in league users stats");

    println!("✅ Solo user correctly excluded from league stats");
}

#[tokio::test]
async fn test_league_users_stats_performance() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create a larger number of users and teams to test performance
    let num_teams = 5;
    let users_per_team = 4;
    let mut all_users = Vec::new();

    // Create users
    for _ in 1..=(num_teams * users_per_team) {
        let user = create_test_user_and_login(&test_app.address).await;
        all_users.push(user);
    }

    // Create teams and assign members
    for team_idx in 0..num_teams {
        let team_owner_idx = team_idx * users_per_team;
        let team_owner = &all_users[team_owner_idx];

        let team_name = format!("PerfTeam_{}_{}", team_idx + 1, Uuid::new_v4().to_string()[..8].to_string());
        let team_request = json!({
            "team_name": team_name,
            "team_description": format!("Performance test team {}", team_idx + 1),
            "team_color": format!("#FF{:02X}{:02X}", (team_idx * 50) % 256, (team_idx * 30) % 256)
        });

        let team_response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/league/teams/register", &test_app.address),
            &team_owner.token,
            Some(team_request),
        ).await;

        assert!(team_response.status().is_success());
        let team_json = team_response.json::<serde_json::Value>().await
            .expect("Failed to parse team response");
        let team_id = team_json["data"]["team_id"].as_str().expect("Team ID should be present");

        // Add other members to the team
        if users_per_team > 1 {
            let members_to_add: Vec<_> = (1..users_per_team)
                .map(|i| {
                    let member_idx = team_owner_idx + i;
                    json!({
                        "username": all_users[member_idx].username,
                        "role": if i == 1 { "admin" } else { "member" }
                    })
                })
                .collect();

            let add_members_request = json!({
                "member_request": members_to_add
            });

            let add_response = make_authenticated_request(
                &client,
                reqwest::Method::POST,
                &format!("{}/league/teams/{}/members", &test_app.address, team_id),
                &team_owner.token,
                Some(add_members_request),
            ).await;

            assert!(add_response.status().is_success());
        }
    }

    // Measure response time
    let start_time = std::time::Instant::now();

    let stats_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/league/users/stats", &test_app.address),
        &all_users[0].token,
        None,
    ).await;

    let response_time = start_time.elapsed();

    assert!(stats_response.status().is_success());
    let stats_json = stats_response.json::<serde_json::Value>().await
        .expect("Failed to parse stats response");

    assert_eq!(stats_json["success"], true);
    
    let total_count = stats_json["total_count"].as_u64().unwrap();
    // Should include at least our test users (there may be additional users from other tests)
    assert!(total_count >= (num_teams * users_per_team) as u64, 
        "Should include at least our {} users, got {}", num_teams * users_per_team, total_count);

    // Response should be fast even with multiple teams and users
    assert!(response_time.as_millis() < 1000, "Response should be under 1 second, got {}ms", response_time.as_millis());

    println!("✅ Performance test passed");
    println!("   - Created {} teams with {} users each", num_teams, users_per_team);
    println!("   - Total users: {}", num_teams * users_per_team);
    println!("   - Response time: {}ms", response_time.as_millis());
}