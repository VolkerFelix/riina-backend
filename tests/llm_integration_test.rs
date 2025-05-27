// tests/llm_integration_test.rs
use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

mod common;
use common::utils::spawn_app;

#[tokio::test]
async fn test_generate_twin_thought_success() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();

    // Register and login user
    let (username, token) = setup_test_user(&client, &test_app.address).await;

    // Act - Generate a twin thought
    let thought_request = json!({
        "trigger": "random",
        "context": {
            "world_state": "balanced",
            "health_score": 75
        }
    });

    let response = client
        .post(&format!("{}/llm/generate_thought", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&thought_request)
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(200, response.status().as_u16());
    
    let response_body: serde_json::Value = response.json().await
        .expect("Failed to parse response as JSON");
    
    // Validate response structure
    assert!(response_body.get("id").is_some());
    assert!(response_body.get("content").is_some());
    assert!(response_body.get("mood").is_some());
    assert!(response_body.get("message_type").is_some());
    assert!(response_body.get("suggested_responses").is_some());
    
    // Validate content is not empty
    let content = response_body["content"].as_str().unwrap();
    assert!(!content.is_empty());
    
    println!("Twin thought generated: {}", content);
}

#[tokio::test]
async fn test_handle_user_response_creates_follow_up() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let (username, token) = setup_test_user(&client, &test_app.address).await;

    // First generate a twin thought
    let thought_request = json!({
        "trigger": "random",
        "context": {}
    });

    let thought_response = client
        .post(&format!("{}/llm/generate_thought", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&thought_request)
        .send()
        .await
        .expect("Failed to generate initial thought");

    assert!(thought_response.status().is_success());

    // Act - Send user response
    let user_response = json!({
        "response_id": "curious",
        "response_text": "Tell me more about that!",
        "conversation_id": Uuid::new_v4()
    });

    let response = client
        .post(&format!("{}/llm/user_response", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&user_response)
        .send()
        .await
        .expect("Failed to execute user response request");

    // Assert
    assert_eq!(200, response.status().as_u16());
    
    let response_body: serde_json::Value = response.json().await
        .expect("Failed to parse response as JSON");
    
    // Validate follow-up response
    assert!(response_body.get("content").is_some());
    let content = response_body["content"].as_str().unwrap();
    assert!(!content.is_empty());
    
    println!("Twin follow-up: {}", content);
}

#[tokio::test]
async fn test_get_conversation_history() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let (username, token) = setup_test_user(&client, &test_app.address).await;

    // Generate some conversation
    let thought_request = json!({
        "trigger": "random",
        "context": {}
    });

    let _thought_response = client
        .post(&format!("{}/llm/generate_thought", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&thought_request)
        .send()
        .await
        .expect("Failed to generate thought");

    // Act - Get conversation history
    let response = client
        .get(&format!("{}/llm/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get conversation history");

    // Assert
    assert_eq!(200, response.status().as_u16());
    
    let response_body: serde_json::Value = response.json().await
        .expect("Failed to parse response as JSON");
    
    assert!(response_body.get("conversation_history").is_some());
    assert!(response_body.get("conversation_stats").is_some());
    
    let history = response_body["conversation_history"].as_array().unwrap();
    assert!(!history.is_empty(), "Should have at least one message in history");
    
    let stats = &response_body["conversation_stats"];
    assert!(stats.get("total_messages").is_some());
    assert!(stats.get("relationship_stage").is_some());
    assert!(stats.get("engagement_level").is_some());
    
    println!("Conversation stats: {}", serde_json::to_string_pretty(stats).unwrap());
}

#[tokio::test]
async fn test_update_user_reaction() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let (username, token) = setup_test_user(&client, &test_app.address).await;

    // Generate a twin thought to react to
    let thought_request = json!({
        "trigger": "random",
        "context": {}
    });

    let thought_response = client
        .post(&format!("{}/llm/generate_thought", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&thought_request)
        .send()
        .await
        .expect("Failed to generate thought");

    let thought_body: serde_json::Value = thought_response.json().await
        .expect("Failed to parse thought response");
    
    let message_id = thought_body["id"].as_str().unwrap();

    // Act - Update user reaction
    let reaction_request = json!({
        "message_id": message_id,
        "reaction_type": "positive",
        "engagement_score": 0.9,
        "response_time_seconds": 5
    });

    let response = client
        .post(&format!("{}/llm/reaction", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&reaction_request)
        .send()
        .await
        .expect("Failed to update reaction");

    // Assert
    assert_eq!(200, response.status().as_u16());
    
    let response_body: serde_json::Value = response.json().await
        .expect("Failed to parse response as JSON");
    
    assert_eq!(response_body["status"], "success");
}

#[tokio::test]
async fn test_trigger_health_reaction() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let (username, token) = setup_test_user(&client, &test_app.address).await;

    // Act - Trigger health reaction
    let health_data = json!({
        "health_score": 85,
        "energy_score": 90,
        "stress_score": 30,
        "cognitive_score": 80,
        "world_state": "active",
        "recent_changes": [
            {
                "metric": "steps",
                "old_value": 5000.0,
                "new_value": 10000.0,
                "change_type": "improvement"
            }
        ]
    });

    let response = client
        .post(&format!("{}/llm/health_reaction", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&health_data)
        .send()
        .await
        .expect("Failed to trigger health reaction");

    // Assert
    assert_eq!(200, response.status().as_u16());
    
    let response_body: serde_json::Value = response.json().await
        .expect("Failed to parse response as JSON");
    
    assert!(response_body.get("content").is_some());
    let content = response_body["content"].as_str().unwrap();
    assert!(!content.is_empty());
    
    // Should be a reaction type message for health data
    let message_type = response_body["message_type"].as_str().unwrap();
    println!("Expected: Reaction, Got: {}", message_type);
    println!("Response content: {}", content);
    
    // Check if it's the correct message type
    if message_type != "Reaction" {
        // Print debug info to help diagnose the issue
        println!("Full response: {}", serde_json::to_string_pretty(&response_body).unwrap());
    }
    
    assert_eq!(message_type, "Reaction", "Health reactions should have message_type 'Reaction'");
    
    println!("Health reaction: {}", content);
}

#[tokio::test]
async fn test_personality_evolution_over_time() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let (username, token) = setup_test_user(&client, &test_app.address).await;

    // Generate multiple interactions with negative reactions to test personality adaptation
    for i in 0..3 {
        // Generate thought
        let thought_request = json!({
            "trigger": "random",
            "context": {}
        });

        let thought_response = client
            .post(&format!("{}/llm/generate_thought", &test_app.address))
            .header("Authorization", format!("Bearer {}", token))
            .json(&thought_request)
            .send()
            .await
            .expect("Failed to generate thought");

        let thought_body: serde_json::Value = thought_response.json().await
            .expect("Failed to parse thought response");
        
        let message_id = thought_body["id"].as_str().unwrap();

        // Add negative reaction to trigger personality adjustment
        let reaction_request = json!({
            "message_id": message_id,
            "reaction_type": "dismissive",
            "engagement_score": 0.2,
            "response_time_seconds": 1
        });

        let _reaction_response = client
            .post(&format!("{}/llm/reaction", &test_app.address))
            .header("Authorization", format!("Bearer {}", token))
            .json(&reaction_request)
            .send()
            .await
            .expect("Failed to update reaction");

        // Small delay between interactions
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Act - Get updated conversation stats to see personality changes
    let response = client
        .get(&format!("{}/llm/history", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get conversation history");

    // Assert
    assert_eq!(200, response.status().as_u16());
    
    let response_body: serde_json::Value = response.json().await
        .expect("Failed to parse response as JSON");
    
    let stats = &response_body["conversation_stats"];
    
    // Engagement should be low due to negative reactions
    let engagement_level = stats["engagement_level"].as_f64().unwrap();
    assert!(engagement_level < 0.5, "Engagement level should be low after negative reactions");
    
    println!("Final conversation stats after personality evolution test: {}", 
             serde_json::to_string_pretty(stats).unwrap());
}

#[tokio::test]
async fn test_different_world_states_generate_different_responses() {
    // Arrange
    let test_app = spawn_app().await;
    let client = Client::new();
    let (username, token) = setup_test_user(&client, &test_app.address).await;

    let world_states = vec!["stressed", "sedentary", "sleepy", "active", "balanced"];
    let mut responses = Vec::new();

    // Act - Generate thoughts for different world states
    for world_state in world_states {
        let thought_request = json!({
            "trigger": "random",
            "context": {
                "world_state": world_state,
                "health_score": 70
            }
        });

        let response = client
            .post(&format!("{}/llm/generate_thought", &test_app.address))
            .header("Authorization", format!("Bearer {}", token))
            .json(&thought_request)
            .send()
            .await
            .expect("Failed to generate thought");

        assert_eq!(200, response.status().as_u16());
        
        let response_body: serde_json::Value = response.json().await
            .expect("Failed to parse response");
        
        let content = response_body["content"].as_str().unwrap().to_string();
        responses.push((world_state, content));

        // Small delay between requests
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Assert - Responses should be different for different world states
    for (world_state, content) in &responses {
        println!("World state '{}': {}", world_state, content);
    }

    // Verify we got unique responses (basic check)
    let unique_responses: std::collections::HashSet<_> = responses.iter().map(|(_, content)| content).collect();
    assert!(unique_responses.len() > 1, "Should generate different responses for different world states");
}

#[tokio::test]
async fn test_llm_fallback_when_service_unavailable() {
    // This test assumes the LLM service is not running
    // The system should gracefully fall back to hardcoded responses
    
    let test_app = spawn_app().await;
    let client = Client::new();
    let (username, token) = setup_test_user(&client, &test_app.address).await;

    // Act - Try to generate thought (should use fallback)
    let thought_request = json!({
        "trigger": "random",
        "context": {
            "world_state": "balanced"
        }
    });

    let response = client
        .post(&format!("{}/llm/generate_thought", &test_app.address))
        .header("Authorization", format!("Bearer {}", token))
        .json(&thought_request)
        .send()
        .await
        .expect("Failed to execute request");

    // Assert - Should still return a valid response (fallback)
    assert_eq!(200, response.status().as_u16());
    
    let response_body: serde_json::Value = response.json().await
        .expect("Failed to parse response as JSON");
    
    assert!(response_body.get("content").is_some());
    let content = response_body["content"].as_str().unwrap();
    assert!(!content.is_empty());
    
    // Check if it's using fallback (could check metadata)
    let metadata = &response_body["metadata"];
    let model_used = metadata["model_used"].as_str().unwrap();
    println!("Model used: {}", model_used);
    
    // Fallback responses should still have suggested responses
    let suggested_responses = response_body["suggested_responses"].as_array().unwrap();
    assert!(!suggested_responses.is_empty());
}

// Helper function to set up a test user and return username and token
async fn setup_test_user(client: &Client, base_url: &str) -> (String, String) {
    let username = format!("llmtestuser{}", Uuid::new_v4());
    let password = "password123";
    let email = format!("{}@example.com", username);

    // Register user
    let user_request = json!({
        "username": username,
        "password": password,
        "email": email
    });

    let register_response = client
        .post(&format!("{}/register_user", base_url))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to register user");

    assert!(register_response.status().is_success(), "Registration should succeed");

    // Login user
    let login_request = json!({
        "username": username,
        "password": password
    });

    let login_response = client
        .post(&format!("{}/login", base_url))
        .json(&login_request)
        .send()
        .await
        .expect("Failed to login user");

    assert!(login_response.status().is_success(), "Login should succeed");
    
    let login_json = login_response.json::<serde_json::Value>().await
        .expect("Failed to parse login response as JSON");
    let token = login_json["token"].as_str().expect("Token not found in response").to_string();

    (username, token)
}