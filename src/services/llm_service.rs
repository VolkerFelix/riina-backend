use std::time::{Duration, Instant};
use reqwest::Client;
use tokio_retry::{strategy::FixedInterval, Retry};
use uuid::Uuid;
use chrono::Utc;

use crate::models::llm::{
    TwinRequest, TwinResponse, LLMError, TwinMessageType, 
    ResponseMetadata, SuggestedResponse
};

#[derive(Clone)]
pub struct LLMService {
    client: Client,
    base_url: String,
    timeout: Duration,
    max_retries: usize,
}

impl LLMService {
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url,
            timeout: Duration::from_secs(25),
            max_retries: 3,
        }
    }

    /// Generate twin response with retry logic
    pub async fn generate_twin_response(&self, request: TwinRequest) -> Result<TwinResponse, LLMError> {
        let start_time = Instant::now();
        
        let retry_strategy = FixedInterval::from_millis(1000).take(self.max_retries);
        
        let result = Retry::spawn(retry_strategy, || {
            self.call_llm_service(request.clone())
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

    /// Make actual HTTP call to LLM service
    async fn call_llm_service(&self, request: TwinRequest) -> Result<TwinResponse, LLMError> {
        tracing::debug!("Calling LLM service for user: {}", request.user_id);

        let response = self.client
            .post(&format!("{}/generate_twin_response", self.base_url))
            .json(&request)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LLMError::Timeout
                } else {
                    LLMError::NetworkError(e)
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            
            return Err(match status.as_u16() {
                429 => LLMError::RateLimited,
                500..=599 => LLMError::ServiceUnavailable(error_text),
                _ => LLMError::InvalidResponse(format!("HTTP {}: {}", status, error_text))
            });
        }

        let llm_response: TwinResponse = response.json().await?;
        
        // Validate response
        if llm_response.content.is_empty() {
            return Err(LLMError::InvalidResponse("Empty content".to_string()));
        }

        Ok(llm_response)
    }

    /// Health check for LLM service
    pub async fn health_check(&self) -> bool {
        match self.client
            .get(&format!("{}/health", self.base_url))
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// Generate a fallback response when LLM service is unavailable
    fn generate_fallback_response(&self, request: &TwinRequest) -> TwinResponse {
        let world_state = &request.health_context.world_state;
        
        let (content, mood) = match world_state.as_str() {
            "stressed" => (
                "I'm feeling a bit scattered right now... like my thoughts are doing jumping jacks! 🤸‍♂️",
                "scattered"
            ),
            "sedentary" => (
                "I've been practicing my advanced couch-sitting techniques. I'm getting really good at it! 🛋️",
                "lazy"
            ),
            "sleepy" => (
                "*yawn* Did someone say something? I was just resting my eyes... for the past hour... 😴",
                "drowsy"
            ),
            "active" => (
                "I'm feeling AMAZING! Like I could run a marathon... or at least think about running one! 🏃‍♂️",
                "energetic"
            ),
            _ => (
                "Just hanging out, living my best digital life! How are you doing? 😊",
                "content"
            )
        };

        TwinResponse {
            id: Uuid::new_v4(),
            content: content.to_string(),
            mood: mood.to_string(),
            message_type: TwinMessageType::ThoughtBubble,
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

    /// Quick response for immediate reactions (like health data updates)
    pub async fn generate_quick_reaction(&self, request: TwinRequest) -> Result<TwinResponse, LLMError> {
        // For quick reactions, we want faster turnaround, so shorter timeout
        let quick_client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create quick HTTP client");

        let response = quick_client
            .post(&format!("{}/generate_quick_reaction", self.base_url))
            .json(&request)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                Ok(resp.json().await?)
            }
            _ => {
                // Quick fallback for immediate reactions
                Ok(self.generate_quick_fallback_reaction(&request))
            }
        }
    }

    fn generate_quick_fallback_reaction(&self, request: &TwinRequest) -> TwinResponse {
        let content = match &request.trigger {
            crate::models::llm::TwinTrigger::HealthDataUpdate => {
                "Ooh, new data! Let me process this... 🤔"
            }
            crate::models::llm::TwinTrigger::WorldStateChange { from: _, to } => {
                match to.as_str() {
                    "active" => "Woah! I'm feeling the energy! ⚡",
                    "stressed" => "Things are getting a bit intense in here... 😵‍💫",
                    "sleepy" => "Everything is getting... so... sleepy... 😴",
                    _ => "Something's changing in my world... 🌍"
                }
            }
            _ => "Something interesting just happened! 🎉"
        };

        TwinResponse {
            id: Uuid::new_v4(),
            content: content.to_string(),
            mood: "processing".to_string(),
            message_type: TwinMessageType::Reaction,
            suggested_responses: vec![],
            personality_evolution: None,
            metadata: ResponseMetadata {
                generated_at: Utc::now(),
                model_used: "quick_fallback".to_string(),
                generation_time_ms: 0,
                confidence_score: Some(0.9),
                context_tokens: None,
            },
        }
    }
}