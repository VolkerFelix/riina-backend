// tests/team_registration_test.rs
use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

mod common;
use common::utils::spawn_app;

#[tokio::test]
async fn test_team_registration_flow() {
    // Set up the test app
    let test_app = spawn_app().await;
    let client = Client::new();

    // Step 1: Register a user
    let username = format!("teamowner{}", Uuid::new_v4());
    let password = "password123";
    let email = format!("{}@example.com", username);

    let user_request = json!({
        "username": username,
        "password": password,
        "email": email
    });

    let response = client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to register user");

    assert!(response.status().is_success(), "User registration should succeed");

    // Step 2: Login to get JWT token
    let login_request = json!({
        "username": username,
        "password": password
    });

    let login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&login_request)
        .send()
        .await
        .expect("Failed to login");

    assert!(login_response.status().is_success(), "Login should succeed");
    
    let login_json = login_response.json::<serde_json::Value>().await
        .expect("Failed to parse login response");
    let token = login_json["token"].as_str().expect("Token not found");

    println!("✅ User registered and authenticated");

    // Step 3: Register a team
    let team_name = format!("Test Team {}", Uuid::new_v4().to_string()[..8].to_string());
    let team_request = json!({
        "team_name": team_name,
        "team_description": "A fantastic test team ready for action!",
        "team_color": "#FF6B35"
    });

    let team_response = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&team_request)
        .send()
        .await
        .expect("Failed to register team");

    assert!(team_response.status().is_success(), "Team registration should succeed");
    
    let team_json = team_response.json::<serde_json::Value>().await
        .expect("Failed to parse team response");

    assert_eq!(team_json["success"], true);
    let team_id = team_json["data"]["team_id"].as_str()
        .expect("Team ID should be present");
    
    println!("✅ Team registered with ID: {}", team_id);

    // Step 4: Verify team was created in database
    let saved_team = sqlx::query!(
        "SELECT team_name, team_description, team_color FROM teams WHERE id = $1",
        Uuid::parse_str(team_id).unwrap()
    )
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to fetch saved team");

    assert_eq!(saved_team.team_name, team_name);
    assert_eq!(saved_team.team_description, Some("A fantastic test team ready for action!".to_string()));
    assert_eq!(saved_team.team_color, "#FF6B35");

    println!("✅ Team data verified in database");

    // Step 5: Get team information via API
    let get_team_response = client
        .get(&format!("{}/league/teams/{}", &test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get team info");

    assert!(get_team_response.status().is_success(), "Get team info should succeed");
    
    let team_info_json = get_team_response.json::<serde_json::Value>().await
        .expect("Failed to parse team info response");

    assert_eq!(team_info_json["success"], true);
    assert_eq!(team_info_json["data"]["team_name"], team_name);
    assert_eq!(team_info_json["data"]["owner_username"], username);

    println!("✅ Team information retrieved successfully");

    // Step 6: Update team information
    let update_request = json!({
        "team_description": "An even more fantastic test team!",
        "team_color": "#00FF00"
    });

    let update_response = client
        .put(&format!("{}/league/teams/{}", &test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", token))
        .json(&update_request)
        .send()
        .await
        .expect("Failed to update team");

    assert!(update_response.status().is_success(), "Team update should succeed");

    // Verify update in database
    let updated_team = sqlx::query!(
        "SELECT team_description, team_color FROM teams WHERE id = $1",
        Uuid::parse_str(team_id).unwrap()
    )
    .fetch_one(&test_app.db_pool)
    .await
    .expect("Failed to fetch updated team");

    assert_eq!(updated_team.team_description, Some("An even more fantastic test team!".to_string()));
    assert_eq!(updated_team.team_color, "#00FF00");

    println!("✅ Team updated successfully");

    // Step 7: Try to register another team with same user (should fail)
    let duplicate_team_request = json!({
        "team_name": "Another Team",
        "team_description": "This should not work",
        "team_color": "#0000FF"
    });

    let duplicate_response = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&duplicate_team_request)
        .send()
        .await
        .expect("Failed to attempt duplicate team registration");

    assert_eq!(duplicate_response.status(), 409, "Duplicate team registration should fail with 409 Conflict");

    println!("✅ Duplicate team registration properly rejected");

    // Step 8: Get all teams
    let all_teams_response = client
        .get(&format!("{}/league/teams?limit=10", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get all teams");

    assert!(all_teams_response.status().is_success(), "Get all teams should succeed");
    
    let all_teams_json = all_teams_response.json::<serde_json::Value>().await
        .expect("Failed to parse all teams response");

    assert_eq!(all_teams_json["success"], true);
    let teams_array = all_teams_json["data"].as_array()
        .expect("Data should be an array");
    
    // Should have at least our team
    assert!(teams_array.len() >= 1, "Should have at least one team");
    
    // Find our team in the list
    let our_team = teams_array.iter()
        .find(|team| team["team_name"] == team_name)
        .expect("Our team should be in the list");

    assert_eq!(our_team["owner_username"], username);

    println!("✅ All teams retrieved successfully");

    println!("\n🎉 TEAM REGISTRATION TEST COMPLETED SUCCESSFULLY!");
    println!("===================================================");
    println!("✅ User registration and authentication");
    println!("✅ Team registration");
    println!("✅ Database persistence verification");
    println!("✅ Team information retrieval");
    println!("✅ Team information updates");
    println!("✅ Duplicate registration prevention");
    println!("✅ Team listing functionality");
}

#[tokio::test]
async fn test_team_registration_validation() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create and authenticate a user
    let username = format!("validator{}", Uuid::new_v4());
    let password = "password123";
    let email = format!("{}@example.com", username);

    let user_request = json!({
        "username": username,
        "password": password,
        "email": email
    });

    client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to register user");

    let login_request = json!({
        "username": username,
        "password": password
    });

    let login_response = client
        .post(&format!("{}/login", &test_app.address))
        .json(&login_request)
        .send()
        .await
        .expect("Failed to login");

    let login_json = login_response.json::<serde_json::Value>().await
        .expect("Failed to parse login response");
    let token = login_json["token"].as_str().expect("Token not found");

    // Test various validation scenarios
    
    // Test 1: Empty team name
    let empty_name_request = json!({
        "team_name": "",
        "team_description": "Valid description"
    });

    let response = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&empty_name_request)
        .send()
        .await
        .expect("Failed to send empty name request");

    assert_eq!(response.status(), 400, "Empty team name should be rejected");

    // Test 2: Team name too short
    let short_name_request = json!({
        "team_name": "A",
        "team_description": "Valid description"
    });

    let response = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&short_name_request)
        .send()
        .await
        .expect("Failed to send short name request");

    assert_eq!(response.status(), 400, "Short team name should be rejected");

    // Test 3: Team name too long
    let long_name_request = json!({
        "team_name": "A".repeat(60),
        "team_description": "Valid description"
    });

    let response = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&long_name_request)
        .send()
        .await
        .expect("Failed to send long name request");

    assert_eq!(response.status(), 400, "Long team name should be rejected");

    // Test 4: Invalid team color
    let invalid_color_request = json!({
        "team_name": "Valid Team Name",
        "team_color": "not-a-color"
    });

    let response = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&invalid_color_request)
        .send()
        .await
        .expect("Failed to send invalid color request");

    assert_eq!(response.status(), 400, "Invalid team color should be rejected");

    // Test 5: Valid team registration (should succeed)
    let unique_team_name = format!("Team {}", Uuid::new_v4().to_string()[..8].to_string());
    let valid_request = json!({
        "team_name": unique_team_name,
        "team_description": "A team for testing validation",
        "team_color": "#32CD32"
    });

    let response = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&valid_request)
        .send()
        .await
        .expect("Failed to send valid request");

    assert!(response.status().is_success(), "Valid team registration should succeed");

    println!("✅ Team registration validation tests passed");
}

#[tokio::test]
async fn test_team_name_uniqueness() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Create two users
    let user1_name = format!("user1_{}", Uuid::new_v4());
    let user2_name = format!("user2_{}", Uuid::new_v4());
    let password = "password123";

    // Register user 1
    let user1_request = json!({
        "username": user1_name,
        "password": password,
        "email": format!("{}@example.com", user1_name)
    });

    client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user1_request)
        .send()
        .await
        .expect("Failed to register user 1");

    // Register user 2
    let user2_request = json!({
        "username": user2_name,
        "password": password,
        "email": format!("{}@example.com", user2_name)
    });

    client
        .post(&format!("{}/register_user", &test_app.address))
        .json(&user2_request)
        .send()
        .await
        .expect("Failed to register user 2");

    // Login both users
    let login_request1 = json!({
        "username": user1_name,
        "password": password
    });

    let login_response1 = client
        .post(&format!("{}/login", &test_app.address))
        .json(&login_request1)
        .send()
        .await
        .expect("Failed to login user 1");

    let token1 = login_response1.json::<serde_json::Value>().await
        .expect("Failed to parse login response 1")["token"]
        .as_str().expect("Token not found").to_string();

    let login_request2 = json!({
        "username": user2_name,
        "password": password
    });

    let login_response2 = client
        .post(&format!("{}/login", &test_app.address))
        .json(&login_request2)
        .send()
        .await
        .expect("Failed to login user 2");

    let token2 = login_response2.json::<serde_json::Value>().await
        .expect("Failed to parse login response 2")["token"]
        .as_str().expect("Token not found").to_string();

    // User 1 registers a team
    let unique_team_name = format!("Team_{}", Uuid::new_v4().to_string()[..8].to_string());
    let team_request = json!({
        "team_name": unique_team_name.clone(),
        "team_description": "First team with this name",
        "team_color": "#FF0000"
    });

    let response1 = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .header("Authorization", format!("Bearer {}", token1))
        .json(&team_request)
        .send()
        .await
        .expect("Failed to register team for user 1");

    assert!(response1.status().is_success(), "First team registration should succeed");

    // User 2 tries to register a team with the same name (should fail)
    let duplicate_team_request = json!({
        "team_name": unique_team_name,
        "team_description": "Second team with same name",
        "team_color": "#0000FF"
    });

    let response2 = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .header("Authorization", format!("Bearer {}", token2))
        .json(&duplicate_team_request)
        .send()
        .await
        .expect("Failed to attempt duplicate team registration");

    assert_eq!(response2.status(), 409, "Duplicate team name should be rejected with 409 Conflict");

    // User 2 registers with a different name (should succeed)
    let different_team_name = format!("Team_{}", Uuid::new_v4().to_string()[..8].to_string());
    let different_team_request = json!({
        "team_name": different_team_name,
        "team_description": "Team with unique name",
        "team_color": "#00FF00"
    });

    let response3 = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .header("Authorization", format!("Bearer {}", token2))
        .json(&different_team_request)
        .send()
        .await
        .expect("Failed to register different team");

    assert!(response3.status().is_success(), "Different team name should succeed");

    println!("✅ Team name uniqueness tests passed");
}

#[tokio::test]
async fn test_unauthorized_team_operations() {
    let test_app = spawn_app().await;
    let client = Client::new();

    // Try to register a team without authentication
    let team_request = json!({
        "team_name": "Unauthorized Team",
        "team_description": "This should fail"
    });

    let response = client
        .post(&format!("{}/league/teams/register", &test_app.address))
        .json(&team_request)
        .send()
        .await
        .expect("Failed to send unauthorized request");

    assert_eq!(response.status(), 401, "Unauthorized team registration should fail");

    // Try to get teams without authentication
    let response = client
        .get(&format!("{}/league/teams", &test_app.address))
        .send()
        .await
        .expect("Failed to send unauthorized get request");

    assert_eq!(response.status(), 401, "Unauthorized team listing should fail");

    // Try to update a team without authentication
    let update_request = json!({
        "team_name": "Updated Name"
    });

    let response = client
        .put(&format!("{}/league/teams/{}", &test_app.address, Uuid::new_v4()))
        .json(&update_request)
        .send()
        .await
        .expect("Failed to send unauthorized update request");

    assert_eq!(response.status(), 401, "Unauthorized team update should fail");

    println!("✅ Unauthorized access tests passed");
}