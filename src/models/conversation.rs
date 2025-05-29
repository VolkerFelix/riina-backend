use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;

/// Comprehensive conversation context stored in Redis
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConversationContext {
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub total_messages: usize,
    pub conversation_summary: ConversationSummary,
    pub recent_messages: Vec<ConversationMessage>,
    pub twin_personality: TwinPersonalityState,
    pub relationship_metrics: RelationshipMetrics,
    pub conversation_themes: Vec<ConversationTheme>,
}

/// High-level summary of the conversation relationship
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConversationSummary {
    pub relationship_stage: RelationshipStage,
    pub dominant_topics: Vec<String>,
    pub user_preferences: UserPreferences,
    pub conversation_style: ConversationStyle,
    pub memorable_moments: Vec<MemorableMoment>,
}

/// Current relationship stage between user and twin
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RelationshipStage {
    FirstMeeting,     // 0-5 messages
    GettingAcquainted, // 6-20 messages
    Developing,       // 21-50 messages
    Familiar,         // 51-100 messages
    Close,           // 100+ messages
    Expert,          // 500+ messages with high engagement
}

/// Individual conversation message with rich context
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConversationMessage {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub message_type: MessageType,
    pub sender: MessageSender,
    pub content: String,
    pub mood: Option<String>,
    pub context_tags: Vec<String>,
    pub user_reaction: Option<UserReaction>,
    pub twin_confidence: Option<f64>,
    pub message_intent: Option<MessageIntent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MessageSender {
    Twin,
    User,
    System,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MessageType {
    ThoughtBubble,
    Reaction,
    Mission,
    UserResponse,
    SystemNotification,
    HealthDataReaction,
    PersonalityEvolution,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MessageIntent {
    Humor,
    Encouragement,
    Education,
    CheckIn,
    Celebration,
    Concern,
    Question,
    Storytelling,
}

/// User's reaction to twin messages
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserReaction {
    pub reaction_type: UserReactionType,
    pub response_time_seconds: Option<u64>,
    pub engagement_score: f64, // 0.0 - 1.0
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum UserReactionType {
    Positive,    // Laughed, agreed, engaged positively
    Neutral,     // Acknowledged, simple response
    Negative,    // Disagreed, ignored, negative response
    Curious,     // Asked questions, wanted to know more
    Dismissive,  // Short responses, changing topic
}

/// Twin's evolving personality state
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TwinPersonalityState {
    pub humor_level: i32,        // 1-10
    pub sarcasm_level: i32,      // 1-10
    pub enthusiasm_level: i32,   // 1-10
    pub empathy_level: i32,      // 1-10
    pub directness_level: i32,   // 1-10
    pub encouragement_style: EncouragementStyle,
    pub communication_quirks: Vec<String>,
    pub learned_user_preferences: HashMap<String, String>,
    pub personality_evolution_log: Vec<PersonalityChange>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum EncouragementStyle {
    Gentle,
    Enthusiastic,
    ToughLove,
    Humorous,
    Scientific,
    Supportive,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PersonalityChange {
    pub timestamp: DateTime<Utc>,
    pub trait_name: String,
    pub old_value: i32,
    pub new_value: i32,
    pub trigger_reason: String,
    pub confidence: f64,
}

/// Relationship quality metrics
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RelationshipMetrics {
    pub trust_level: f64,           // 0.0 - 1.0
    pub engagement_level: f64,      // 0.0 - 1.0
    pub humor_compatibility: f64,   // 0.0 - 1.0
    pub conversation_depth: f64,    // 0.0 - 1.0
    pub response_frequency: f64,    // messages per day
    pub avg_response_time: f64,     // seconds
    pub total_conversation_time: u64, // seconds
}

/// User preferences learned from conversation
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserPreferences {
    pub preferred_communication_time: Vec<String>, // "morning", "evening", etc.
    pub humor_tolerance: f64,       // 0.0 - 1.0
    pub detail_preference: DetailLevel,
    pub encouragement_preference: EncouragementStyle,
    pub topic_interests: HashMap<String, f64>, // topic -> interest score
    pub conversation_length_preference: ConversationLength,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DetailLevel {
    Brief,      // Short, concise responses
    Moderate,   // Balanced detail
    Detailed,   // Comprehensive explanations
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ConversationLength {
    Quick,      // 1-2 exchanges
    Standard,   // 3-5 exchanges  
    Extended,   // 6+ exchanges
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ConversationStyle {
    Casual,
    Professional, 
    Playful,
    Supportive,
    Educational,
    Mixed,
}

/// Recurring conversation themes
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConversationTheme {
    pub theme_name: String,
    pub frequency: usize,
    pub user_engagement: f64,
    pub twin_success_rate: f64,
    pub last_mentioned: DateTime<Utc>,
    pub related_health_data: Vec<String>,
}

/// Memorable conversation moments
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemorableMoment {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub moment_type: MomentType,
    pub description: String,
    pub impact_score: f64, // How significant this moment was
    pub related_messages: Vec<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MomentType {
    FirstLaugh,
    Breakthrough,
    DeepConnection,
    HealthWin,
    PersonalShare,
    Conflict,
    Resolution,
    Milestone,
}

// Default implementations
impl Default for TwinPersonalityState {
    fn default() -> Self {
        Self {
            humor_level: 7,
            sarcasm_level: 4,
            enthusiasm_level: 6,
            empathy_level: 7,
            directness_level: 5,
            encouragement_style: EncouragementStyle::Supportive,
            communication_quirks: vec![
                "loves coffee metaphors".to_string(),
                "uses tech analogies".to_string(),
            ],
            learned_user_preferences: HashMap::new(),
            personality_evolution_log: vec![],
        }
    }
}

impl Default for RelationshipMetrics {
    fn default() -> Self {
        Self {
            trust_level: 0.5,
            engagement_level: 0.5,
            humor_compatibility: 0.5,
            conversation_depth: 0.3,
            response_frequency: 0.0,
            avg_response_time: 0.0,
            total_conversation_time: 0,
        }
    }
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            preferred_communication_time: vec!["any".to_string()],
            humor_tolerance: 0.7,
            detail_preference: DetailLevel::Moderate,
            encouragement_preference: EncouragementStyle::Supportive,
            topic_interests: HashMap::new(),
            conversation_length_preference: ConversationLength::Standard,
        }
    }
}

impl RelationshipStage {
    /// Determine relationship stage based on message count and engagement
    pub fn from_message_count_and_engagement(message_count: usize, avg_engagement: f64) -> Self {
        match message_count {
            0..=5 => RelationshipStage::FirstMeeting,
            6..=20 => RelationshipStage::GettingAcquainted,
            21..=50 => RelationshipStage::Developing,
            51..=100 => RelationshipStage::Familiar,
            101..=500 => RelationshipStage::Close,
            _ => {
                if avg_engagement > 0.8 {
                    RelationshipStage::Expert
                } else {
                    RelationshipStage::Close
                }
            }
        }
    }
    
    pub fn as_str(&self) -> &'static str {
        match self {
            RelationshipStage::FirstMeeting => "first_meeting",
            RelationshipStage::GettingAcquainted => "getting_acquainted", 
            RelationshipStage::Developing => "developing",
            RelationshipStage::Familiar => "familiar",
            RelationshipStage::Close => "close",
            RelationshipStage::Expert => "expert",
        }
    }
}

impl ConversationContext {
    /// Create new conversation context for a user
    pub fn new(user_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            user_id,
            created_at: now,
            last_updated: now,
            total_messages: 0,
            conversation_summary: ConversationSummary {
                relationship_stage: RelationshipStage::FirstMeeting,
                dominant_topics: vec![],
                user_preferences: UserPreferences::default(),
                conversation_style: ConversationStyle::Casual,
                memorable_moments: vec![],
            },
            recent_messages: vec![],
            twin_personality: TwinPersonalityState::default(),
            relationship_metrics: RelationshipMetrics::default(),
            conversation_themes: vec![],
        }
    }
    
    /// Add a new message to the conversation context
    pub fn add_message(&mut self, message: ConversationMessage) {
        self.recent_messages.push(message);
        self.total_messages += 1;
        self.last_updated = Utc::now();
        
        // Keep only the last 50 messages in recent_messages
        if self.recent_messages.len() > 50 {
            self.recent_messages.remove(0);
        }
        
        // Update relationship stage
        let avg_engagement = self.calculate_average_engagement();
        self.conversation_summary.relationship_stage = 
            RelationshipStage::from_message_count_and_engagement(self.total_messages, avg_engagement);
    }
    
    /// Calculate average user engagement
    fn calculate_average_engagement(&self) -> f64 {
        let engagement_scores: Vec<f64> = self.recent_messages
            .iter()
            .filter_map(|msg| msg.user_reaction.as_ref().map(|r| r.engagement_score))
            .collect();
            
        if engagement_scores.is_empty() {
            0.5 // Default neutral engagement
        } else {
            engagement_scores.iter().sum::<f64>() / engagement_scores.len() as f64
        }
    }
    
    /// Get conversation context for LLM prompt
    pub fn get_context_for_llm(&self) -> String {
        format!(
            "Relationship Stage: {}\nTotal Messages: {}\nEngagement Level: {:.2}\nPersonality: Humor({}), Empathy({}), Directness({})\nUser Prefers: {:?} style, {} detail level",
            self.conversation_summary.relationship_stage.as_str(),
            self.total_messages,
            self.calculate_average_engagement(),
            self.twin_personality.humor_level,
            self.twin_personality.empathy_level,
            self.twin_personality.directness_level,
            self.conversation_summary.conversation_style,
            match self.conversation_summary.user_preferences.detail_preference {
                DetailLevel::Brief => "brief",
                DetailLevel::Moderate => "moderate", 
                DetailLevel::Detailed => "detailed",
            }
        )
    }
}