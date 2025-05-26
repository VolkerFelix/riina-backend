use std::time::Duration;
use tokio::time::{interval, sleep};
use uuid::Uuid;

use crate::services::conversation_service::ConversationService;

/// Background jobs for conversation management
pub struct ConversationJobs {
    conversation_service: ConversationService,
}

impl ConversationJobs {
    pub fn new(conversation_service: ConversationService) -> Self {
        Self { conversation_service }
    }

    /// Start all background jobs
    pub async fn start_all(&self) {
        let service = self.conversation_service.clone();
        
        // Spawn cleanup job
        let cleanup_service = service.clone();
        tokio::spawn(async move {
            Self::cleanup_job(cleanup_service).await;
        });

        // Spawn personality evolution job
        let evolution_service = service.clone();
        tokio::spawn(async move {
            Self::personality_evolution_job(evolution_service).await;
        });

        // Spawn analytics job
        let analytics_service = service.clone();
        tokio::spawn(async move {
            Self::analytics_job(analytics_service).await;
        });
    }

    /// Clean up old conversation data
    async fn cleanup_job(conversation_service: ConversationService) {
        let mut interval = interval(Duration::from_secs(24 * 60 * 60)); // Run daily
        
        loop {
            interval.tick().await;
            
            tracing::info!("Starting conversation cleanup job");
            
            match conversation_service.cleanup_old_conversations(30).await {
                Ok(cleaned_count) => {
                    tracing::info!("Cleaned up {} old conversation messages", cleaned_count);
                }
                Err(e) => {
                    tracing::error!("Conversation cleanup failed: {:?}", e);
                }
            }
        }
    }

    /// Periodic personality evolution based on patterns
    async fn personality_evolution_job(_conversation_service: ConversationService) {
        let mut interval = interval(Duration::from_secs(6 * 60 * 60)); // Run every 6 hours
        
        loop {
            interval.tick().await;
            
            tracing::info!("Starting personality evolution job");
            
            // This would need a way to get all active users
            // For now, we'll skip this and let personality evolution happen
            // during real-time conversations
            
            sleep(Duration::from_secs(30)).await;
        }
    }

    /// Generate conversation analytics and insights
    async fn analytics_job(_conversation_service: ConversationService) {
        let mut interval = interval(Duration::from_secs(12 * 60 * 60)); // Run twice daily
        
        loop {
            interval.tick().await;
            
            tracing::info!("Starting conversation analytics job");
            
            // Generate aggregated insights
            // This could include:
            // - Most successful conversation patterns
            // - Personality trait effectiveness
            // - User engagement trends
            // - Optimal timing for different types of messages
            
            sleep(Duration::from_secs(10)).await;
        }
    }
}

/// Proactive conversation triggers
pub struct ConversationTriggers {
    conversation_service: ConversationService,
}

impl ConversationTriggers {
    pub fn new(conversation_service: ConversationService) -> Self {
        Self { conversation_service }
    }

    /// Check if a user might need a check-in
    pub async fn should_trigger_checkin(&self, user_id: &Uuid) -> Result<bool, Box<dyn std::error::Error>> {
        let context = self.conversation_service.get_or_create_context(user_id).await?;
        
        // Trigger check-in if:
        // 1. Haven't talked in 24+ hours
        // 2. Last engagement was low
        // 3. User seems to be in a concerning health state
        
        let hours_since_last_message = context.recent_messages
            .last()
            .map(|msg| {
                let now = chrono::Utc::now();
                (now - msg.timestamp).num_hours()
            })
            .unwrap_or(48); // Default to 48 hours if no messages

        let should_checkin = hours_since_last_message >= 24 ||
            context.relationship_metrics.engagement_level < 0.3 ||
            context.relationship_metrics.trust_level < 0.3;

        Ok(should_checkin)
    }

    /// Determine optimal message timing for a user
    pub async fn get_optimal_message_time(&self, user_id: &Uuid) -> Result<Option<chrono::NaiveTime>, Box<dyn std::error::Error>> {
        let context = self.conversation_service.get_or_create_context(user_id).await?;
        
        // Analyze when user is most responsive
        let response_times: Vec<(chrono::NaiveTime, f64)> = context.recent_messages
            .iter()
            .filter_map(|msg| {
                msg.user_reaction.as_ref().map(|reaction| {
                    (msg.timestamp.time(), reaction.engagement_score)
                })
            })
            .collect();

        if response_times.is_empty() {
            return Ok(None);
        }

        // Find time with highest average engagement
        // This is a simplified version - you could implement more sophisticated analysis
        let best_time = response_times
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(time, _)| *time);

        Ok(best_time)
    }
}

/// Conversation pattern analyzer
pub struct ConversationAnalyzer;

impl ConversationAnalyzer {
    /// Analyze what makes conversations successful
    pub fn analyze_success_patterns(context: &crate::models::conversation::ConversationContext) -> ConversationInsights {
        let total_messages = context.recent_messages.len();
        if total_messages == 0 {
            return ConversationInsights::default();
        }

        let humor_success_rate = Self::calculate_humor_success_rate(context);
        let avg_engagement = Self::calculate_average_engagement(context);
        let conversation_length_preference = Self::analyze_conversation_length_preference(context);
        let preferred_topics = Self::identify_preferred_topics(context);
        let optimal_personality_settings = Self::suggest_personality_adjustments(context);

        ConversationInsights {
            humor_success_rate,
            avg_engagement,
            conversation_length_preference,
            preferred_topics,
            optimal_personality_settings,
            total_analyzed_messages: total_messages,
        }
    }

    fn calculate_humor_success_rate(context: &crate::models::conversation::ConversationContext) -> f64 {
        let humor_messages: Vec<_> = context.recent_messages
            .iter()
            .filter(|msg| msg.context_tags.contains(&"humor".to_string()))
            .collect();

        if humor_messages.is_empty() {
            return 0.5;
        }

        let successful_humor = humor_messages
            .iter()
            .filter(|msg| {
                msg.user_reaction
                    .as_ref()
                    .map(|r| r.engagement_score > 0.7)
                    .unwrap_or(false)
            })
            .count();

        successful_humor as f64 / humor_messages.len() as f64
    }

    fn calculate_average_engagement(context: &crate::models::conversation::ConversationContext) -> f64 {
        let engagement_scores: Vec<f64> = context.recent_messages
            .iter()
            .filter_map(|msg| msg.user_reaction.as_ref().map(|r| r.engagement_score))
            .collect();

        if engagement_scores.is_empty() {
            0.5
        } else {
            engagement_scores.iter().sum::<f64>() / engagement_scores.len() as f64
        }
    }

    fn analyze_conversation_length_preference(context: &crate::models::conversation::ConversationContext) -> String {
        // Analyze patterns in conversation lengths
        // This is simplified - you could track conversation sessions
        let avg_engagement = Self::calculate_average_engagement(context);
        
        if avg_engagement > 0.8 {
            "extended".to_string()
        } else if avg_engagement > 0.6 {
            "standard".to_string()
        } else {
            "brief".to_string()
        }
    }

    fn identify_preferred_topics(context: &crate::models::conversation::ConversationContext) -> Vec<String> {
        // Analyze which topics get the best engagement
        let mut topic_engagement: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();

        for msg in &context.recent_messages {
            if let Some(reaction) = &msg.user_reaction {
                for tag in &msg.context_tags {
                    topic_engagement
                        .entry(tag.clone())
                        .or_insert_with(Vec::new)
                        .push(reaction.engagement_score);
                }
            }
        }

        // Get topics with highest average engagement
        let mut topics: Vec<(String, f64)> = topic_engagement
            .into_iter()
            .map(|(topic, scores)| {
                let avg = scores.iter().sum::<f64>() / scores.len() as f64;
                (topic, avg)
            })
            .collect();

        topics.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        topics.into_iter().take(5).map(|(topic, _)| topic).collect()
    }

    fn suggest_personality_adjustments(context: &crate::models::conversation::ConversationContext) -> PersonalityAdjustments {
        let humor_success = Self::calculate_humor_success_rate(context);
        let avg_engagement = Self::calculate_average_engagement(context);

        PersonalityAdjustments {
            humor_adjustment: if humor_success < 0.3 { -1 } else if humor_success > 0.8 { 1 } else { 0 },
            empathy_adjustment: if avg_engagement < 0.4 { 1 } else { 0 },
            directness_adjustment: if avg_engagement < 0.3 { -1 } else { 0 },
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConversationInsights {
    pub humor_success_rate: f64,
    pub avg_engagement: f64,
    pub conversation_length_preference: String,
    pub preferred_topics: Vec<String>,
    pub optimal_personality_settings: PersonalityAdjustments,
    pub total_analyzed_messages: usize,
}

#[derive(Debug, Clone)]
pub struct PersonalityAdjustments {
    pub humor_adjustment: i32,      // -2 to +2
    pub empathy_adjustment: i32,    // -2 to +2
    pub directness_adjustment: i32, // -2 to +2
}

impl Default for ConversationInsights {
    fn default() -> Self {
        Self {
            humor_success_rate: 0.5,
            avg_engagement: 0.5,
            conversation_length_preference: "standard".to_string(),
            preferred_topics: vec![],
            optimal_personality_settings: PersonalityAdjustments {
                humor_adjustment: 0,
                empathy_adjustment: 0,
                directness_adjustment: 0,
            },
            total_analyzed_messages: 0,
        }
    }
}