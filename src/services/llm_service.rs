use std::time::Instant;
use tokio_retry::{strategy::FixedInterval, Retry};
use uuid::Uuid;
use chrono::Utc;

use crate::models::llm::{
    TwinRequest, TwinResponse, LLMError, TwinMessageType, 
    ResponseMetadata, SuggestedResponse, TwinTrigger
};
use crate::services::ollama::ollama::Ollama;

#[derive(Clone)]
pub struct LLMService {
    ollama: Ollama,
    max_retries: usize,
}

impl LLMService {
    pub fn new(model_name: String, base_url: String) -> Self {
        Self {
            ollama: Ollama::new(model_name, base_url),
            max_retries: 3,
        }
    }

    /// Health check for LLM service
    pub async fn health_check(&self) -> bool {
        self.ollama.health_check().await
    }

    /// Generate twin response with retry logic
    pub async fn generate_twin_response(&self, request: TwinRequest) -> Result<TwinResponse, LLMError> {
        let start_time = Instant::now();
        
        let retry_strategy = FixedInterval::from_millis(1000).take(self.max_retries);
        
        let result = Retry::spawn(retry_strategy, || {
            self.ollama.call(request.clone())
        }).await;

        match result {
            Ok(mut response) => {
                response.metadata.generation_time_ms = start_time.elapsed().as_millis() as u64;
                Ok(response)
            }
            Err(e) => {
                tracing::error!("LLM service call failed after {} retries: {:?}", self.max_retries, e);
                
                // Return fallback response if LLM service is down
                Ok(self.generate_fallback_response(&request))
            }
        }
    }

    /// Generate a fallback response when LLM service is unavailable
    fn generate_fallback_response(&self, request: &TwinRequest) -> TwinResponse {
        let world_state = &request.health_context.world_state;
        
        tracing::debug!("Generating fallback response for world state: {}", world_state);
        
        // Determine message type based on trigger
        let message_type = match &request.trigger {
            TwinTrigger::HealthDataUpdate => TwinMessageType::Reaction,
            TwinTrigger::UserMessage(_) => TwinMessageType::ThoughtBubble,
            TwinTrigger::RandomThought => TwinMessageType::ThoughtBubble,
            TwinTrigger::WorldStateChange { .. } => TwinMessageType::Reaction,
            TwinTrigger::MissionCompleted(_) => TwinMessageType::Celebration,
            TwinTrigger::TimeBasedCheck => TwinMessageType::CheckIn,
        };
        
        let (content, mood) = match (&request.trigger, world_state.as_str()) {
            // Health data update reactions
            (TwinTrigger::HealthDataUpdate, "stressed") => (
                "Whoa! I can feel the stress waves radiating through our connection! 😵‍💫 Let's find some calm in this storm.",
                "concerned"
            ),
            (TwinTrigger::HealthDataUpdate, "sedentary") => (
                "I see we're in full couch-potato mode! 🥔 My circuits are getting a bit sluggish too...",
                "lazy"
            ),
            (TwinTrigger::HealthDataUpdate, "sleepy") => (
                "Detecting major drowsiness levels! 😴 Even my digital eyelids are getting heavy...",
                "drowsy"
            ),
            (TwinTrigger::HealthDataUpdate, "active") => (
                "ENERGY SURGE DETECTED! ⚡ I'm practically bouncing off the digital walls right now!",
                "energetic"
            ),
            (TwinTrigger::HealthDataUpdate, "balanced") => (
                "Ahh, perfect harmony! 🧘‍♀️ Everything feels just right in our world.",
                "content"
            ),
            (TwinTrigger::HealthDataUpdate, _) => (
                "New health data incoming! 📊 Let me process these fascinating biometrics...",
                "curious"
            ),
            
            // World state change reactions
            (TwinTrigger::WorldStateChange { from, to }, _) => {
                match (from.as_str(), to.as_str()) {
                    (_, "active") => ("Woah! Energy levels are SOARING! 🚀", "energetic"),
                    (_, "stressed") => ("Uh oh, things are getting intense in here... 😰", "worried"),
                    (_, "sleepy") => ("Everything is getting... so... sleepy... 😴", "drowsy"),
                    (_, "balanced") => ("Ahh, much better! Balance restored! ⚖️", "relieved"),
                    _ => ("Something's shifting in my world... 🌍", "curious")
                }
            },
            
            // User message responses
            (TwinTrigger::UserMessage(_), _) => (
                "I hear you! Let me think about that... 🤔",
                "thoughtful"
            ),
            
            // Mission completed
            (TwinTrigger::MissionCompleted(_), _) => (
                "Mission accomplished! 🎉 We make a great team!",
                "celebratory"
            ),
            
            // Time-based check-ins
            (TwinTrigger::TimeBasedCheck, _) => (
                "Just checking in! How are things going? 👋",
                "friendly"
            ),
            
            // Random thoughts based on world state - THIS IS THE KEY PART
            (TwinTrigger::RandomThought, "stressed") => (
                "I'm feeling a bit scattered right now... like my thoughts are doing jumping jacks! 🤸‍♂️",
                "scattered"
            ),
            (TwinTrigger::RandomThought, "sedentary") => (
                "I've been practicing my advanced couch-sitting techniques. I'm getting really good at it! 🛋️",
                "lazy"
            ),
            (TwinTrigger::RandomThought, "sleepy") => (
                "*yawn* Did someone say something? I was just resting my eyes... for the past hour... 😴",
                "drowsy"
            ),
            (TwinTrigger::RandomThought, "active") => (
                "I'm feeling AMAZING! Like I could run a marathon... or at least think about running one! 🏃‍♂️",
                "energetic"
            ),
            (TwinTrigger::RandomThought, "balanced") => (
                "Everything's in perfect harmony right now! It's like digital zen! ✨",
                "content"
            ),
            // Make sure to handle any unknown world states for random thoughts
            (TwinTrigger::RandomThought, _) => (
                "Hmm, I'm in an interesting state: {}... Let me contemplate this! 🤔",
                "curious"
            ),
        };

        tracing::debug!("Generated fallback content for {}: {}", world_state, content);

        TwinResponse {
            id: Uuid::new_v4(),
            content: content.to_string(),
            mood: mood.to_string(),
            message_type,
            suggested_responses: vec![
                SuggestedResponse {
                    id: "response_1".to_string(),
                    text: "Tell me more about how you're feeling".to_string(),
                    tone: "curious".to_string(),
                    leads_to: Some("deeper_conversation".to_string()),
                },
                SuggestedResponse {
                    id: "response_2".to_string(),
                    text: "That's interesting!".to_string(),
                    tone: "supportive".to_string(),
                    leads_to: Some("encouragement".to_string()),
                },
                SuggestedResponse {
                    id: "response_3".to_string(),
                    text: "What should we focus on today?".to_string(),
                    tone: "practical".to_string(),
                    leads_to: Some("mission_planning".to_string()),
                },
            ],
            personality_evolution: None,
            metadata: ResponseMetadata {
                generated_at: Utc::now(),
                model_used: "fallback".to_string(),
                generation_time_ms: 0,
                confidence_score: Some(0.8),
                context_tokens: None,
            },
        }
    }
}