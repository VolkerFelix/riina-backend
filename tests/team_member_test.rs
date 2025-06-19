use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

mod common;
use common::utils::spawn_app;

#[tokio::test]
async fn test_add_user_to_team_success() {
    let test_app = spawn_app().await;
    let client = Client::new();
    
    // Create first user (team owner)
    let owner_username = format!("team_owner_{}", Uuid::new_v4());
    let owner_user = json!({
        "username": owner_username,
        "email": format!("{}@example.com", owner_username),
        "password": "password123"
    });
    
    let response = client
        .post(&format!("{}/register_user", test_app.address))
        .json(&owner_user)
        .send()
        .await
        .expect("Failed to execute registration request");
    assert_eq!(response.status().as_u16(), 200);
    
    // Create 4 more users(to be added as members)
    let mut member_usernames = Vec::new();
    for _ in 1..5 {
        let member_username = format!("team_member_{}", Uuid::new_v4());
        let member_user = json!({
            "username": member_username,
            "email": format!("{}@example.com", member_username),
            "password": "password123"
        });

        let response = client
            .post(&format!("{}/register_user", test_app.address))
            .json(&member_user)
            .send()
            .await
            .expect("Failed to execute registration request");
        assert_eq!(response.status().as_u16(), 200);
        
        member_usernames.push(member_username);
    }
    
    // Login as owner
    let login_data = json!({
        "username": owner_username,
        "password": "password123"
    });
    
    let login_response = client
        .post(&format!("{}/login", test_app.address))
        .json(&login_data)
        .send()
        .await
        .expect("Failed to execute login request");
    assert_eq!(login_response.status().as_u16(), 200);
    
    let login_body: serde_json::Value = login_response.json().await.unwrap();
    let owner_token = login_body["token"].as_str().unwrap();
    
    // Register a team
    let team_name = format!("Test_Team_{}", Uuid::new_v4());
    let team_data = json!({
        "team_name": team_name,
        "team_description": "A test team",
        "team_color": "#FF0000"
    });
    
    let response = client
        .post(&format!("{}/league/teams/register", test_app.address))
        .header("Authorization", format!("Bearer {}", owner_token))
        .json(&team_data)
        .send()
        .await
        .expect("Failed to execute team registration request");
    assert_eq!(response.status().as_u16(), 201);
    
    let team_body: serde_json::Value = response.json().await.unwrap();
    let team_id = team_body["data"]["team_id"].as_str().unwrap();
    
    // Add members to team
    let add_member_data = json!({
        "member_request": member_usernames.iter().map(|username| json!({
            "username": username,
            "role": "member"
        })).collect::<Vec<_>>()
    });
    
    let response = client
        .post(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner_token))
        .json(&add_member_data)
        .send()
        .await
        .expect("Failed to execute add team members request");
    assert_eq!(response.status().as_u16(), 201);
    
    let member_body: serde_json::Value = response.json().await.unwrap();
    assert!(member_body["success"].as_bool().unwrap());
    
    // Verify all members were added
    let members = member_body["members"].as_array().unwrap();
    assert_eq!(members.len(), 4);
    for (i, member) in members.iter().enumerate() {
        assert_eq!(member["username"].as_str().unwrap(), member_usernames[i]);
        assert_eq!(member["role"].as_str().unwrap(), "member");
    }
}

#[tokio::test]
async fn test_get_team_members() {
    let test_app = spawn_app().await;
    let client = Client::new();
    
    // Create owner
    let owner_username = format!("team_owner_{}", Uuid::new_v4());
    let owner_user = json!({
        "username": owner_username,
        "email": format!("{}@example.com", owner_username),
        "password": "password123"
    });
    
    let response = client
        .post(&format!("{}/register_user", test_app.address))
        .json(&owner_user)
        .send()
        .await
        .expect("Failed to execute registration request");
    assert_eq!(response.status().as_u16(), 200);
    
    // Login and create team
    let login_data = json!({
        "username": owner_username,
        "password": "password123"
    });
    
    let login_response = client
        .post(&format!("{}/login", test_app.address))
        .json(&login_data)
        .send()
        .await
        .expect("Failed to execute login request");
    
    let login_body: serde_json::Value = login_response.json().await.unwrap();
    let owner_token = login_body["token"].as_str().unwrap();
    
    let team_name = format!("Test_Team_{}", Uuid::new_v4());
    let team_data = json!({
        "team_name": team_name,
        "team_description": "A test team"
    });
    
    let response = client
        .post(&format!("{}/league/teams/register", test_app.address))
        .header("Authorization", format!("Bearer {}", owner_token))
        .json(&team_data)
        .send()
        .await
        .expect("Failed to execute team registration request");
    
    let team_body: serde_json::Value = response.json().await.unwrap();
    let team_id = team_body["data"]["team_id"].as_str().unwrap();
    
    // Get team members (should include owner automatically)
    let response = client
        .get(&format!("{}/league/teams/{}/members", test_app.address, team_id))
        .header("Authorization", format!("Bearer {}", owner_token))
        .send()
        .await
        .expect("Failed to execute get team members request");
    assert_eq!(response.status().as_u16(), 200);
    
    let members_body: serde_json::Value = response.json().await.unwrap();
    assert!(members_body["success"].as_bool().unwrap());
    assert_eq!(members_body["data"]["member_count"].as_u64().unwrap(), 1);
    
    let members = members_body["data"]["members"].as_array().unwrap();
    assert_eq!(members[0]["username"].as_str().unwrap(), owner_username);
    assert_eq!(members[0]["role"].as_str().unwrap(), "owner");
}