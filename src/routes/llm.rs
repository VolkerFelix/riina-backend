// src/routes/llm.rs
use actix_web::{web, HttpResponse, Result, post, get};
use uuid::Uuid;
use serde_json::json;
use chrono::Utc;

use crate::middleware::auth::Claims;
use crate::models::llm::{
    TwinRequest, HealthContext, TwinTrigger, TwinPersonality, 
    TwinWebSocketMessage
};
use crate::services::llm_service::LLMService;
use crate::services::conversation_service::ConversationService;
use crate::models::conversation::{
    ConversationMessage, MessageSender, MessageType, MessageIntent, UserReaction, UserReactionType
};
use crate::handlers::llm_handler::{generate_twin_thought, GenerateThoughtRequest};

#[derive(serde::Deserialize)]
pub struct UserResponseRequest {
    pub response_id: String,
    pub response_text: String,
    pub conversation_id: Uuid,
}

#[derive(serde::Deserialize)]
pub struct UpdateReactionRequest {
    pub message_id: Uuid,
    pub reaction_type: String, // "positive", "negative", "neutral", "curious", "dismissive"
    pub engagement_score: f64, // 0.0 - 1.0
    pub response_time_seconds: Option<u64>,
}

/// Generate a twin thought bubble
#[post("/generate_thought")]
pub async fn generate_thought(
    req: web::Json<GenerateThoughtRequest>,
    llm_service: web::Data<LLMService>,
    conversation_service: web::Data<ConversationService>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    generate_twin_thought(req, llm_service, conversation_service, claims).await
}

/// Handle user response to twin message
#[post("/user_response")]
pub async fn handle_user_response(
    req: web::Json<UserResponseRequest>,
    llm_service: web::Data<LLMService>,
    conversation_service: web::Data<ConversationService>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "error": "Invalid user ID"
            })));
        }
    };

    tracing::info!("Processing user response from: {}", user_id);

    // Create user response message
    let user_message = ConversationMessage {
        id: Uuid::new_v4(),
        timestamp: Utc::now(),
        message_type: MessageType::UserResponse,
        sender: MessageSender::User,
        content: req.response_text.clone(),
        mood: None,
        context_tags: vec!["user_response".to_string()],
        user_reaction: None,
        twin_confidence: None,
        message_intent: None,
    };

    // Store user response
    if let Err(e) = conversation_service.add_message(&user_id, user_message).await {
        tracing::error!("Failed to store user response: {:?}", e);
    }

    // Generate follow-up response (simplified for now)
    let health_context = get_user_health_context(&user_id).await.unwrap_or_default();
    
    let twin_request = TwinRequest {
        user_id,
        health_context,
        trigger: TwinTrigger::UserMessage(req.response_text.clone()),
        conversation_history: vec![], // Simplified
        twin_personality: TwinPersonality::default(),
    };

    match llm_service.generate_twin_response(twin_request).await {
        Ok(response) => {
            // Store follow-up conversation message
            let conversation_message = ConversationMessage {
                id: response.id,
                timestamp: Utc::now(),
                message_type: MessageType::ThoughtBubble,
                sender: MessageSender::Twin,
                content: response.content.clone(),
                mood: Some(response.mood.clone()),
                context_tags: vec!["follow_up".to_string()],
                user_reaction: None,
                twin_confidence: response.metadata.confidence_score,
                message_intent: Some(MessageIntent::Encouragement),
            };

            if let Err(e) = conversation_service.add_message(&user_id, conversation_message).await {
                tracing::error!("Failed to store twin follow-up: {:?}", e);
            }

            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to generate follow-up response: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "error": "Failed to generate follow-up response"
            })))
        }
    }
}

/// Get twin conversation history
#[get("/history")]
pub async fn get_twin_history(
    conversation_service: web::Data<ConversationService>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "error": "Invalid user ID"
            })));
        }
    };

    match conversation_service.get_recent_messages(&user_id, 50).await {
        Ok(messages) => {
            let stats = conversation_service.get_conversation_stats(&user_id).await
                .unwrap_or_else(|_| crate::services::conversation_service::ConversationStats {
                    total_messages: 0,
                    relationship_stage: "first_meeting".to_string(),
                    engagement_level: 0.5,
                    trust_level: 0.5,
                    humor_level: 7,
                    empathy_level: 7,
                    personality_changes: 0,
                });

            Ok(HttpResponse::Ok().json(json!({
                "conversation_history": messages,
                "conversation_stats": stats,
                "total_messages": messages.len()
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get conversation history: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "error": "Failed to get conversation history"
            })))
        }
    }
}

/// Update user reaction to a twin message
#[post("/reaction")]
pub async fn update_user_reaction(
    req: web::Json<UpdateReactionRequest>,
    conversation_service: web::Data<ConversationService>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "error": "Invalid user ID"
            })));
        }
    };

    let reaction_type = match req.reaction_type.as_str() {
        "positive" => UserReactionType::Positive,
        "negative" => UserReactionType::Negative,
        "neutral" => UserReactionType::Neutral,
        "curious" => UserReactionType::Curious,
        "dismissive" => UserReactionType::Dismissive,
        _ => UserReactionType::Neutral,
    };

    let user_reaction = UserReaction {
        reaction_type,
        response_time_seconds: req.response_time_seconds,
        engagement_score: req.engagement_score.clamp(0.0, 1.0),
    };

    match conversation_service.update_user_reaction(&user_id, &req.message_id, user_reaction).await {
        Ok(_) => {
            Ok(HttpResponse::Ok().json(json!({
                "status": "success",
                "message": "User reaction updated"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to update user reaction: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "error": "Failed to update user reaction"
            })))
        }
    }
}

/// Trigger a health-based twin reaction
#[post("/health_reaction")]
pub async fn trigger_health_reaction(
    health_data: web::Json<serde_json::Value>,
    llm_service: web::Data<LLMService>,
    conversation_service: web::Data<ConversationService>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "error": "Invalid user ID"
            })));
        }
    };

    tracing::info!("Triggering health reaction for user: {}", user_id);

    // Extract health context from the data
    let health_context = extract_health_context_from_data(&health_data)?;

    let twin_request = TwinRequest {
        user_id,
        health_context,
        trigger: TwinTrigger::HealthDataUpdate,
        conversation_history: vec![], // Simplified for now
        twin_personality: TwinPersonality::default(),
    };

    match llm_service.generate_twin_response(twin_request).await {
        Ok(mut response) => {
            // Ensure the response has the correct message type for health reactions
            response.message_type = crate::models::llm::TwinMessageType::Reaction;
            
            // Create health reaction message
            let conversation_message = ConversationMessage {
                id: response.id,
                timestamp: Utc::now(),
                message_type: MessageType::HealthDataReaction,
                sender: MessageSender::Twin,
                content: response.content.clone(),
                mood: Some(response.mood.clone()),
                context_tags: vec!["health_reaction".to_string(), "automatic".to_string()],
                user_reaction: None,
                twin_confidence: response.metadata.confidence_score,
                message_intent: Some(MessageIntent::Celebration),
            };

            // Store the conversation message
            if let Err(e) = conversation_service.add_message(&user_id, conversation_message).await {
                tracing::error!("Failed to store health reaction message: {:?}", e);
            }

            // Send via WebSocket
            let ws_message = TwinWebSocketMessage::TwinReaction {
                response: response.clone(),
                trigger_event: "health_data_update".to_string(),
            };
            let _ = conversation_service.publish_twin_message(&user_id, &ws_message).await;

            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to generate health reaction: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "error": "Failed to generate health reaction",
                "details": e.to_string()
            })))
        }
    }
}

// Helper functions

async fn get_user_health_context(_user_id: &Uuid) -> Option<HealthContext> {
    // TODO: Implement based on your health data structure
    // This should pull from your existing health data tables
    Some(HealthContext::default())
}

fn extract_health_context_from_data(health_data: &serde_json::Value) -> Result<HealthContext> {
    // Extract health context from incoming health data
    let health_score = health_data.get("health_score")
        .and_then(|v| v.as_i64())
        .unwrap_or(70) as i32;
    
    let energy_score = health_data.get("energy_score")
        .and_then(|v| v.as_i64())
        .unwrap_or(70) as i32;
    
    let stress_score = health_data.get("stress_score")
        .and_then(|v| v.as_i64())
        .unwrap_or(50) as i32;
    
    let cognitive_score = health_data.get("cognitive_score")
        .and_then(|v| v.as_i64())
        .unwrap_or(70) as i32;

    let world_state = health_data.get("world_state")
        .and_then(|v| v.as_str())
        .unwrap_or("balanced")
        .to_string();

    Ok(HealthContext {
        overall_health: health_score,
        health_score,
        energy_score,
        cognitive_score,
        stress_score,
        world_state,
        recent_changes: vec![], // TODO: Extract from health data
    })
}