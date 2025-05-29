use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use thiserror::Error as ThisError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TwinRequest {
    pub user_id: Uuid,
    pub health_context: HealthContext,
    pub trigger: TwinTrigger,
    pub conversation_history: Vec<ConversationEntry>,
    pub twin_personality: TwinPersonality,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HealthContext {
    pub overall_health: i32,
    pub health_score: i32,
    pub energy_score: i32,
    pub cognitive_score: i32,
    pub stress_score: i32,
    pub world_state: String, // stressed, sedentary, sleepy, active, balanced
    pub recent_changes: Vec<HealthChange>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HealthChange {
    pub metric: String, // "sleep", "steps", "heart_rate", etc.
    pub old_value: f64,
    pub new_value: f64,
    pub change_type: String, // "improvement", "decline", "neutral"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TwinTrigger {
    HealthDataUpdate,
    UserMessage(String),
    RandomThought,
    WorldStateChange { from: String, to: String },
    MissionCompleted(String),
    TimeBasedCheck, // Periodic check-ins
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConversationEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub message_type: MessageType,
    pub content: String,
    pub user_response: Option<String>,
    pub twin_mood: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MessageType {
    TwinThought,
    TwinReaction,
    TwinMission,
    UserResponse,
    SystemEvent,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TwinPersonality {
    pub humor_level: i32, // 1-10
    pub sarcasm_level: i32, // 1-10
    pub encouragement_style: String, // "gentle", "tough-love", "cheerleader", "sarcastic"
    pub relationship_stage: String, // "new", "developing", "familiar", "close"
    pub preferred_topics: Vec<String>,
    pub quirks: Vec<String>, // Things that make this twin unique
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TwinResponse {
    pub id: Uuid,
    pub content: String,
    pub mood: String, // "happy", "concerned", "excited", "sleepy", "stressed", etc.
    pub message_type: TwinMessageType,
    pub suggested_responses: Vec<SuggestedResponse>,
    pub personality_evolution: Option<PersonalityChange>,
    pub metadata: ResponseMetadata,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TwinMessageType {
    ThoughtBubble,
    Reaction,
    Mission,
    CheckIn,
    Celebration,
    Concern,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SuggestedResponse {
    pub id: String,
    pub text: String,
    pub tone: String, // "supportive", "dismissive", "curious", "playful"
    pub leads_to: Option<String>, // What kind of conversation this leads to
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PersonalityChange {
    pub trait_name: String,
    pub old_value: i32,
    pub new_value: i32,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResponseMetadata {
    pub generated_at: DateTime<Utc>,
    pub model_used: String,
    pub generation_time_ms: u64,
    pub confidence_score: Option<f64>,
    pub context_tokens: Option<usize>,
}

// WebSocket message types for twin communication
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum TwinWebSocketMessage {
    #[serde(rename = "twin_thought")]
    TwinThought {
        response: TwinResponse,
        requires_response: bool,
    },
    #[serde(rename = "twin_reaction")]
    TwinReaction {
        response: TwinResponse,
        trigger_event: String,
    },
    #[serde(rename = "twin_mission")]
    TwinMission {
        response: TwinResponse,
        mission_data: Option<serde_json::Value>,
    },
    #[serde(rename = "personality_update")]
    PersonalityUpdate {
        changes: Vec<PersonalityChange>,
        message: String,
    },
    #[serde(rename = "conversation_context")]
    ConversationContext {
        recent_messages: Vec<ConversationEntry>,
        relationship_stage: String,
    },
}

// Error types for LLM service
#[derive(Debug, ThisError)]
pub enum LLMError {
    #[error("LLM service unavailable: {0}")]
    ServiceUnavailable(String),
    
    #[error("Invalid response format: {0}")]
    InvalidResponse(String),
    
    #[error("Request timeout")]
    Timeout,
    
    #[error("Rate limit exceeded")]
    RateLimited,
    
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

impl Default for TwinPersonality {
    fn default() -> Self {
        Self {
            humor_level: 7,
            sarcasm_level: 5,
            encouragement_style: "gentle".to_string(),
            relationship_stage: "new".to_string(),
            preferred_topics: vec!["health".to_string(), "activities".to_string()],
            quirks: vec!["loves coffee references".to_string()],
        }
    }
}

impl Default for HealthContext {
    fn default() -> Self {
        Self {
            overall_health: 70,
            health_score: 70,
            energy_score: 70,
            cognitive_score: 70,
            stress_score: 50,
            world_state: "balanced".to_string(),
            recent_changes: vec![],
        }
    }
}