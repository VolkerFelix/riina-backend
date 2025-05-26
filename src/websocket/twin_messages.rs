use actix::prelude::*;
use actix_web_actors::ws;
use serde_json::json;
use uuid::Uuid;

use crate::models::llm::TwinWebSocketMessage;
use crate::websocket::messages::{WsConnection, ConnectedUser};

/// Handle twin-specific WebSocket messages
impl Handler<TwinWebSocketMessage> for WsConnection {
    type Result = ();

    fn handle(&mut self, msg: TwinWebSocketMessage, ctx: &mut Self::Context) -> Self::Result {
        let message_json = match serde_json::to_string(&msg) {
            Ok(json) => json,
            Err(e) => {
                tracing::error!("Failed to serialize twin message: {:?}", e);
                return;
            }
        };

        tracing::debug!("Sending twin message to WebSocket: {:?}", msg);
        ctx.text(message_json);
    }
}

/// Message to broadcast twin message to specific user
#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastTwinMessage {
    pub user_id: Uuid,
    pub message: TwinWebSocketMessage,
}

/// Handle broadcasting twin messages to connected users
impl Handler<BroadcastTwinMessage> for crate::websocket::server::WsServer {
    type Result = ();

    fn handle(&mut self, msg: BroadcastTwinMessage, _: &mut Self::Context) -> Self::Result {
        tracing::debug!("Broadcasting twin message to user: {}", msg.user_id);

        // Find all sessions for this user
        if let Some(user_sessions) = self.users.get(&msg.user_id) {
            for session_id in user_sessions.iter() {
                if let Some(session) = self.sessions.get(session_id) {
                    let _ = session.addr.do_send(msg.message.clone());
                }
            }
        } else {
            tracing::debug!("No active sessions found for user: {}", msg.user_id);
        }
    }
}

/// Extended WebSocket message types to include twin communication
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "event_type")]
pub enum ExtendedWsMessage {
    // Existing message types
    #[serde(rename = "new_health_data")]
    NewHealthData {
        sync_id: String,
        message: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    #[serde(rename = "connection_established")]
    ConnectionEstablished {
        user_id: Uuid,
        session_id: Uuid,
    },
    
    #[serde(rename = "connection_closed")]
    ConnectionClosed {
        session_id: Uuid,
    },

    // New twin message types
    #[serde(rename = "twin_thought")]
    TwinThought {
        response: crate::models::llm::TwinResponse,
        requires_response: bool,
    },

    #[serde(rename = "twin_reaction")]
    TwinReaction {
        response: crate::models::llm::TwinResponse,
        trigger_event: String,
    },

    #[serde(rename = "twin_mission")]
    TwinMission {
        response: crate::models::llm::TwinResponse,
        mission_data: Option<serde_json::Value>,
    },

    #[serde(rename = "personality_update")]
    PersonalityUpdate {
        changes: Vec<crate::models::llm::PersonalityChange>,
        message: String,
    },

    #[serde(rename = "conversation_context")]
    ConversationContext {
        recent_messages: Vec<crate::models::llm::ConversationEntry>,
        relationship_stage: String,
    },

    // User interaction messages
    #[serde(rename = "user_response")]
    UserResponse {
        conversation_id: Uuid,
        response_id: String,
        response_text: String,
    },

    #[serde(rename = "request_twin_thought")]
    RequestTwinThought {
        trigger: Option<String>,
        context: Option<serde_json::Value>,
    },
}

/// Convert TwinWebSocketMessage to ExtendedWsMessage
impl From<TwinWebSocketMessage> for ExtendedWsMessage {
    fn from(twin_msg: TwinWebSocketMessage) -> Self {
        match twin_msg {
            TwinWebSocketMessage::TwinThought { response, requires_response } => {
                ExtendedWsMessage::TwinThought { response, requires_response }
            }
            TwinWebSocketMessage::TwinReaction { response, trigger_event } => {
                ExtendedWsMessage::TwinReaction { response, trigger_event }
            }
            TwinWebSocketMessage::TwinMission { response, mission_data } => {
                ExtendedWsMessage::TwinMission { response, mission_data }
            }
            TwinWebSocketMessage::PersonalityUpdate { changes, message } => {
                ExtendedWsMessage::PersonalityUpdate { changes, message }
            }
            TwinWebSocketMessage::ConversationContext { recent_messages, relationship_stage } => {
                ExtendedWsMessage::ConversationContext { recent_messages, relationship_stage }
            }
        }
    }
}

/// Handle incoming WebSocket messages from frontend
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                tracing::debug!("Received WebSocket message: {}", text);

                // Try to parse as ExtendedWsMessage
                match serde_json::from_str::<ExtendedWsMessage>(&text) {
                    Ok(extended_msg) => {
                        self.handle_extended_message(extended_msg, ctx);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse WebSocket message: {:?}", e);
                        
                        // Try legacy message format
                        if let Ok(legacy_msg) = serde_json::from_str::<serde_json::Value>(&text) {
                            self.handle_legacy_message(legacy_msg, ctx);
                        }
                    }
                }
            }
            Ok(ws::Message::Binary(_)) => {
                tracing::warn!("Received unexpected binary WebSocket message");
            }
            Ok(ws::Message::Close(reason)) => {
                tracing::info!("WebSocket connection closed: {:?}", reason);
                ctx.stop();
            }
            _ => {}
        }
    }
}

impl WsConnection {
    /// Handle extended WebSocket messages including twin communication
    fn handle_extended_message(&mut self, msg: ExtendedWsMessage, ctx: &mut Self::Context) {
        match msg {
            ExtendedWsMessage::RequestTwinThought { trigger, context } => {
                if let Some(user_id) = self.user_id {
                    // Spawn async task to generate twin thought
                    let server = self.server.clone();
                    let trigger_type = trigger.unwrap_or_else(|| "random".to_string());
                    
                    tokio::spawn(async move {
                        // This would trigger the LLM service
                        // For now, send a simple response
                        let twin_message = TwinWebSocketMessage::TwinThought {
                            response: create_sample_twin_response(&trigger_type),
                            requires_response: true,
                        };
                        
                        server.do_send(BroadcastTwinMessage {
                            user_id,
                            message: twin_message,
                        });
                    });
                } else {
                    tracing::warn!("Received twin thought request from unauthenticated connection");
                }
            }
            
            ExtendedWsMessage::UserResponse { conversation_id, response_id, response_text } => {
                if let Some(user_id) = self.user_id {
                    tracing::info!("User {} responded: {}", user_id, response_text);
                    
                    // This would trigger follow-up conversation
                    // Store user response and generate twin follow-up
                    let server = self.server.clone();
                    
                    tokio::spawn(async move {
                        // Generate follow-up response
                        let follow_up = create_follow_up_response(&response_text);
                        
                        server.do_send(BroadcastTwinMessage {
                            user_id,
                            message: TwinWebSocketMessage::TwinThought {
                                response: follow_up,
                                requires_response: false,
                            },
                        });
                    });
                } else {
                    tracing::warn!("Received user response from unauthenticated connection");
                }
            }
            
            // Handle other message types...
            _ => {
                tracing::debug!("Received extended message: {:?}", msg);
            }
        }
    }

    /// Handle legacy WebSocket messages for backward compatibility
    fn handle_legacy_message(&mut self, msg: serde_json::Value, _ctx: &mut Self::Context) {
        tracing::debug!("Handling legacy WebSocket message: {:?}", msg);
        // Handle any existing message formats
    }
}

/// Create a sample twin response (temporary implementation)
fn create_sample_twin_response(trigger: &str) -> crate::models::llm::TwinResponse {
    use crate::models::llm::*;
    use chrono::Utc;
    use uuid::Uuid;

    let (content, mood) = match trigger {
        "health_update" => (
            "Ooh, new data! *rubs digital hands together* Let me see what's happening in our world! 🤔",
            "curious"
        ),
        "random" => (
            "You know what I was just thinking? If I'm your digital twin, does that mean I get half of your coffee addiction? Because I'm feeling a strong urge for caffeine... ☕",
            "playful"
        ),
        _ => (
            "Hey there! What's on your mind today? 😊",
            "friendly"
        ),
    };

    TwinResponse {
        id: Uuid::new_v4(),
        content: content.to_string(),
        mood: mood.to_string(),
        message_type: TwinMessageType::ThoughtBubble,
        suggested_responses: vec![
            SuggestedResponse {
                id: "curious".to_string(),
                text: "Tell me more about that!".to_string(),
                tone: "curious".to_string(),
                leads_to: Some("deeper_conversation".to_string()),
            },
            SuggestedResponse {
                id: "playful".to_string(),
                text: "Haha, that's funny!".to_string(),
                tone: "playful".to_string(),
                leads_to: Some("humor".to_string()),
            },
            SuggestedResponse {
                id: "practical".to_string(),
                text: "What should we focus on today?".to_string(),
                tone: "practical".to_string(),
                leads_to: Some("goal_setting".to_string()),
            },
        ],
        personality_evolution: None,
        metadata: ResponseMetadata {
            generated_at: Utc::now(),
            model_used: "sample".to_string(),
            generation_time_ms: 50,
            confidence_score: Some(0.9),
            context_tokens: None,
        },
    }
}

/// Create a follow-up response based on user input
fn create_follow_up_response(user_response: &str) -> crate::models::llm::TwinResponse {
    use crate::models::llm::*;
    use chrono::Utc;
    use uuid::Uuid;

    let content = if user_response.to_lowercase().contains("funny") {
        "I'm glad you think so! I've been practicing my digital comedy. My timing might be a bit off since I exist in microseconds, but I'm working on it! 😄"
    } else if user_response.to_lowercase().contains("more") {
        "Well, since you asked... I was actually wondering if digital twins dream of electric sheep? Or in my case, maybe I dream of perfectly balanced macros! 🐑✨"
    } else {
        "I appreciate you sharing that with me! It helps me understand you better. That's what digital twins are for, right? 🤗"
    };

    TwinResponse {
        id: Uuid::new_v4(),
        content: content.to_string(),
        mood: "engaging".to_string(),
        message_type: TwinMessageType::ThoughtBubble,
        suggested_responses: vec![],
        personality_evolution: None,
        metadata: ResponseMetadata {
            generated_at: Utc::now(),
            model_used: "sample_followup".to_string(),
            generation_time_ms: 30,
            confidence_score: Some(0.85),
            context_tokens: None,
        },
    }
}