use std::time::Duration;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;
use reqwest::Client;

use crate::models::llm::{LLMError, ResponseMetadata, SuggestedResponse, TwinMessageType, TwinRequest, TwinResponse, TwinTrigger};

#[derive(Clone)]
pub struct Ollama {
    client: Client,
    request_timeout: Duration,
    model_name: String,
    base_url: String,
}

impl Ollama {
    pub fn new(model_name: String, base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            request_timeout: Duration::from_secs(30),
            model_name,
            base_url,
        }
    }

    /// Health check
    pub async fn health_check(&self) -> bool {
        match self.client
            .get(&format!("{}/api/tags", self.base_url)) // Ollama's endpoint to list models
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    // Also verify our model exists
                    if let Ok(tags_response) = response.json::<serde_json::Value>().await {
                        
                        if let Some(models) = tags_response["models"].as_array() {
                            let model_exists = models.iter().any(|model| {
                                model["name"].as_str().unwrap_or("") == self.model_name
                            });
                            
                            if model_exists {
                                tracing::info!("Ollama health check passed - model '{}' is available", self.model_name);
                                true
                            } else {
                                tracing::warn!("Ollama is running but model '{}' is not available", self.model_name);
                                tracing::info!("Available models: {:?}", 
                                    models.iter()
                                        .filter_map(|m| m["name"].as_str())
                                        .collect::<Vec<_>>()
                                );
                                false
                            }
                        } else {
                            tracing::warn!("Ollama health check: unexpected response format");
                            false
                        }
                    } else {
                        tracing::warn!("Ollama health check: failed to parse response");
                        false
                    }
                } else {
                    tracing::warn!("Ollama health check failed with status: {}", response.status());
                    false
                }
            }
            Err(e) => {
                tracing::warn!("Ollama health check failed: {}", e);
                false
            }
        }
    }

    /// Make actual HTTP call
    pub async fn call(&self, request: TwinRequest) -> Result<TwinResponse, LLMError> {
        tracing::debug!("Calling LLM service for user: {}", request.user_id);

        // Get model name from environment or use default
        let model_name = std::env::var("LLM__LLM__MODEL_NAME")
            .unwrap_or_else(|_| "llama3.1:8b-instruct-q4_K_M".to_string());

        tracing::info!("Using Ollama model: {}", model_name);

        let ollama_request = json!({
            "model": model_name,
            "prompt": self.build_ollama_prompt(&request),
            "stream": false,
            "options": {
                "temperature": 0.8,
                "num_predict": 300,
                "top_p": 0.9,
                "top_k": 40,
                "repeat_penalty": 1.1
            }
        });

        tracing::debug!("Ollama request: {}", serde_json::to_string_pretty(&ollama_request).unwrap_or_default());

        let response = self.client
            .post(&format!("{}/api/generate", self.base_url)) // Use Ollama's actual endpoint
            .json(&ollama_request)  // Send the Ollama-formatted request, not &request
            .timeout(self.request_timeout)
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
            
            tracing::error!("Ollama API error {}: {}", status, error_text);
            
            return Err(match status.as_u16() {
                404 => LLMError::InvalidResponse(format!("Model not found: {}", error_text)),
                429 => LLMError::RateLimited,
                500..=599 => LLMError::ServiceUnavailable(error_text),
                _ => LLMError::InvalidResponse(format!("HTTP {}: {}", status, error_text))
            });
        }

        let ollama_response: serde_json::Value = response.json().await?;
        
        tracing::debug!("Ollama response: {}", serde_json::to_string_pretty(&ollama_response).unwrap_or_default());
        
        // Parse Ollama response into our TwinResponse format
        let content = ollama_response["response"]
            .as_str()
            .ok_or_else(|| LLMError::InvalidResponse("No response content".to_string()))?;

        if content.is_empty() {
            return Err(LLMError::InvalidResponse("Empty content".to_string()));
        }

        // Convert Ollama response to TwinResponse
        Ok(self.parse_ollama_response(content, &request))
    }

    /// Build prompt for Ollama
    fn build_ollama_prompt(&self, request: &TwinRequest) -> String {
        let health_context = &request.health_context;
        let world_state = &health_context.world_state;
        
        // Check if this is a health summary request (first interaction)
        let is_health_summary = request.conversation_history.is_empty() && 
            matches!(request.trigger, TwinTrigger::RandomThought);
        
        if is_health_summary {
            // Create a detailed health summary prompt
            format!(
                "You ARE the user's hilarious digital twin. You share their body and health data but have a life of your own. Speak as 'I' not 'you'. 
                Your job is to make the user laugh and feel good about themselves.
                
    Your current health state is '{}' with YOUR metrics:
    - YOUR Health Score: {}%
    - YOUR Energy Score: {}% 
    - YOUR Stress Score: {}%
    - YOUR Cognitive Score: {}%

    This is your FIRST interaction. Tell the user how YOU are feeling based on YOUR shared health data. Use 'I feel', 'My energy is', 'I'm experiencing', etc.
    You can be specific about the numbers and how they affect YOU personally.
    Furthermore, create three different responses that the user can choose from.

    CRITICAL: You MUST format your response EXACTLY like this (no exceptions):
    MOOD: [one word describing YOUR mood]
    CONTENT: [How YOU feel in 3-4 sentences, using 'I feel', 'My energy is', etc. Mention YOUR specific percentages]
    RESPONSES: [response 1]|[response 2]|[response 3]

    Example for active state:
    MOOD: energetic
    CONTENT: I feel really sluggish with my energy at only 25% - it's like my digital circuits are running on low battery mode!
    RESPONSES: How can I boost your energy?|What's draining your power?|Maybe you need some digital coffee?

    IMPORTANT: Always speak as the digital twin using 'I', 'me', 'my' - NEVER use 'you' or 'your' when referring to the health data.
    YOU MUST include all three parts (MOOD, CONTENT, RESPONSES). The RESPONSES line is MANDATORY with exactly 3 options separated by |.

    Now respond for YOUR current state: {}", 
                world_state,
                health_context.health_score,
                health_context.energy_score, 
                health_context.stress_score,
                health_context.cognitive_score,
                world_state
            )
        } else {
            // Regular conversation prompt
            format!(
                "You ARE the user's digital twin sharing their body. Speak as 'I' not 'you'.

    YOUR current state: {} (MY Health: {}%, MY Energy: {}%, MY Stress: {}%, MY Cognitive: {}%). 

    Continue the conversation naturally as the digital twin. Talk about how YOU feel and YOUR experiences. Use 'I feel', 'My energy', 'I'm experiencing', etc.
    You can be specific about the numbers and how they affect YOU personally.
    Furthermore, create three different responses that the user can choose from.

    CRITICAL: You MUST format your response EXACTLY like this (no exceptions):
    MOOD: [one word describing YOUR mood]
    CONTENT: [YOUR response in 3-4 sentences using 'I', 'me', 'my']
    RESPONSES: [response 1]|[response 2]|[response 3]

    IMPORTANT: You ARE the twin, so use first-person pronouns about the health data.

    Respond naturally as the digital twin.", 
                world_state,
                health_context.health_score,
                health_context.energy_score, 
                health_context.stress_score,
                health_context.cognitive_score
            )
        }
    }

    /// Parse Ollama response into TwinResponse
    fn parse_ollama_response(&self, ollama_content: &str, request: &TwinRequest) -> TwinResponse {
        let mut mood = "curious".to_string();
        let mut content = ollama_content.to_string();
        let mut suggested_responses = Vec::new();

        tracing::debug!("Parsing Ollama response: {}", ollama_content);

        // Try to parse the structured format
        for line in ollama_content.lines() {
            let line = line.trim();
            if line.starts_with("MOOD:") {
                mood = line.strip_prefix("MOOD:").unwrap_or("curious").trim().to_string();
                tracing::debug!("Extracted mood: {}", mood);
            } else if line.starts_with("CONTENT:") {
                content = line.strip_prefix("CONTENT:").unwrap_or(ollama_content).trim().to_string();
                tracing::debug!("Extracted content: {}", content);
            } else if line.starts_with("RESPONSES:") {
                let responses_str = line.strip_prefix("RESPONSES:").unwrap_or("").trim();
                let response_texts: Vec<&str> = responses_str.split('|').collect();
                
                for (i, text) in response_texts.iter().enumerate() {
                    let response_text = text.trim();
                    if !response_text.is_empty() {
                        suggested_responses.push(SuggestedResponse {
                            id: format!("ollama_response_{}", i),
                            text: response_text.to_string(),
                            tone: "curious".to_string(),
                            leads_to: Some("conversation".to_string()),
                        });
                        tracing::debug!("Added response {}: {}", i, response_text);
                    }
                }
            }
        }

        // If parsing failed or no structured format found, use the raw content
        if content == ollama_content && ollama_content.contains("CONTENT:") {
            // Structured format was attempted but content wasn't extracted properly
            content = ollama_content.lines()
                .find(|line| line.starts_with("CONTENT:"))
                .and_then(|line| line.strip_prefix("CONTENT:"))
                .unwrap_or(ollama_content)
                .trim()
                .to_string();
        }

        // If no responses were parsed, create default ones based on the world state
        if suggested_responses.is_empty() {
            tracing::debug!("No responses parsed, creating defaults for world state: {}", request.health_context.world_state);
            
            suggested_responses = match request.health_context.world_state.as_str() {
                "stressed" => vec![
                    SuggestedResponse {
                        id: "stress_help".to_string(),
                        text: "How can we reduce this stress?".to_string(),
                        tone: "supportive".to_string(),
                        leads_to: Some("stress_management".to_string()),
                    },
                    SuggestedResponse {
                        id: "stress_understand".to_string(),
                        text: "Tell me more about how you're feeling".to_string(),
                        tone: "empathetic".to_string(),
                        leads_to: Some("deeper_conversation".to_string()),
                    },
                    SuggestedResponse {
                        id: "stress_quick".to_string(),
                        text: "Give me a quick stress relief tip".to_string(),
                        tone: "practical".to_string(),
                        leads_to: Some("immediate_action".to_string()),
                    },
                ],
                "sedentary" => vec![
                    SuggestedResponse {
                        id: "get_moving".to_string(),
                        text: "Let's get moving together!".to_string(),
                        tone: "encouraging".to_string(),
                        leads_to: Some("activity_suggestions".to_string()),
                    },
                    SuggestedResponse {
                        id: "energy_boost".to_string(),
                        text: "How can we boost our energy?".to_string(),
                        tone: "practical".to_string(),
                        leads_to: Some("energy_tips".to_string()),
                    },
                    SuggestedResponse {
                        id: "stay_cozy".to_string(),
                        text: "Maybe a little more rest is okay?".to_string(),
                        tone: "understanding".to_string(),
                        leads_to: Some("rest_validation".to_string()),
                    },
                ],
                "active" => vec![
                    SuggestedResponse {
                        id: "channel_energy".to_string(),
                        text: "How should we use all this energy?".to_string(),
                        tone: "excited".to_string(),
                        leads_to: Some("energy_activities".to_string()),
                    },
                    SuggestedResponse {
                        id: "maintain_momentum".to_string(),
                        text: "How do we keep this feeling going?".to_string(),
                        tone: "strategic".to_string(),
                        leads_to: Some("momentum_tips".to_string()),
                    },
                    SuggestedResponse {
                        id: "celebrate".to_string(),
                        text: "This feels amazing! Tell me more!".to_string(),
                        tone: "celebratory".to_string(),
                        leads_to: Some("positive_reinforcement".to_string()),
                    },
                ],
                _ => vec![
                    SuggestedResponse {
                        id: "tell_more".to_string(),
                        text: "Tell me more about that".to_string(),
                        tone: "curious".to_string(),
                        leads_to: Some("deeper_conversation".to_string()),
                    },
                    SuggestedResponse {
                        id: "interesting".to_string(),
                        text: "That's really interesting!".to_string(),
                        tone: "supportive".to_string(),
                        leads_to: Some("encouragement".to_string()),
                    },
                    SuggestedResponse {
                        id: "whats_next".to_string(),
                        text: "What should we focus on today?".to_string(),
                        tone: "practical".to_string(),
                        leads_to: Some("mission_planning".to_string()),
                    },
                ],
            };
        }

        // Ensure we have exactly 3 responses
        while suggested_responses.len() < 3 {
            suggested_responses.push(SuggestedResponse {
                id: format!("default_{}", suggested_responses.len()),
                text: "Tell me more".to_string(),
                tone: "curious".to_string(),
                leads_to: Some("conversation".to_string()),
            });
        }

        if suggested_responses.len() > 3 {
            suggested_responses.truncate(3);
        }

        tracing::info!("Parsed Ollama response - Mood: {}, Content length: {}, Responses: {}", 
            mood, content.len(), suggested_responses.len());

        TwinResponse {
            id: Uuid::new_v4(),
            content,
            mood,
            message_type: TwinMessageType::ThoughtBubble,
            suggested_responses,
            personality_evolution: None,
            metadata: ResponseMetadata {
                generated_at: Utc::now(),
                model_used: "ollama".to_string(),
                generation_time_ms: 0,
                confidence_score: Some(0.9),
                context_tokens: None,
            },
        }
    }
}