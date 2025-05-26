use redis::{AsyncCommands, RedisError};
use uuid::Uuid;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::models::conversation::{
    ConversationContext, ConversationMessage, MessageSender, UserReaction, 
    UserReactionType, PersonalityChange
};

#[derive(Clone)]
pub struct ConversationService {
    redis_client: redis::Client,
}

impl ConversationService {
    pub fn new(redis_client: redis::Client) -> Self {
        Self { redis_client }
    }

    /// Get or create conversation context for a user
    pub async fn get_or_create_context(&self, user_id: &Uuid) -> Result<ConversationContext, ConversationError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        
        let context_key = format!("conversation:context:{}", user_id);
        
        // Try to get existing context
        let existing_context: Option<String> = conn.get(&context_key).await?;
        
        match existing_context {
            Some(json_str) => {
                match serde_json::from_str::<ConversationContext>(&json_str) {
                    Ok(context) => Ok(context),
                    Err(e) => {
                        tracing::warn!("Failed to deserialize conversation context for user {}: {}. Creating new context.", user_id, e);
                        let new_context = ConversationContext::new(*user_id);
                        self.save_context(&new_context).await?;
                        Ok(new_context)
                    }
                }
            }
            None => {
                // Create new context
                let new_context = ConversationContext::new(*user_id);
                self.save_context(&new_context).await?;
                Ok(new_context)
            }
        }
    }

    /// Save conversation context to Redis
    pub async fn save_context(&self, context: &ConversationContext) -> Result<(), ConversationError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        
        let context_key = format!("conversation:context:{}", context.user_id);
        let json_str = serde_json::to_string(context)?;
        
        // Set with expiration (30 days)
        let _: () = conn.set_ex(&context_key, json_str, 30 * 24 * 60 * 60).await?;
        
        tracing::debug!("Saved conversation context for user: {}", context.user_id);
        Ok(())
    }

    /// Add a message to the conversation
    pub async fn add_message(&self, user_id: &Uuid, message: ConversationMessage) -> Result<(), ConversationError> {
        let mut context = self.get_or_create_context(user_id).await?;
        
        // Add message to context
        context.add_message(message.clone());
        
        // Save updated context
        self.save_context(&context).await?;
        
        // Also store individual message for detailed history
        self.store_individual_message(user_id, &message).await?;
        
        Ok(())
    }

    /// Store individual message for detailed history
    async fn store_individual_message(&self, user_id: &Uuid, message: &ConversationMessage) -> Result<(), ConversationError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        
        let messages_key = format!("conversation:messages:{}", user_id);
        let message_json = serde_json::to_string(message)?;
        
        // Add to sorted set with timestamp as score
        let timestamp_score = message.timestamp.timestamp() as f64;
        let _: i32 = conn.zadd(&messages_key, message_json, timestamp_score).await?;
        
        // Keep only last 1000 messages
        let _: i32 = conn.zremrangebyrank(&messages_key, 0, -1001).await?;
        
        // Set expiration on the messages set
        let _: bool = conn.expire(&messages_key, 30 * 24 * 60 * 60).await?; // 30 days
        
        Ok(())
    }

    /// Get recent messages for context
    pub async fn get_recent_messages(&self, user_id: &Uuid, limit: usize) -> Result<Vec<ConversationMessage>, ConversationError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        
        let messages_key = format!("conversation:messages:{}", user_id);
        
        // Get most recent messages from sorted set
        let message_jsons: Vec<String> = conn.zrevrange(&messages_key, 0, limit as isize - 1).await?;
        
        let mut messages = Vec::new();
        for json_str in message_jsons {
            match serde_json::from_str::<ConversationMessage>(&json_str) {
                Ok(message) => messages.push(message),
                Err(e) => {
                    tracing::warn!("Failed to deserialize message: {}", e);
                }
            }
        }
        
        // Reverse to get chronological order (oldest first)
        messages.reverse();
        Ok(messages)
    }

    /// Update user reaction to a twin message
    pub async fn update_user_reaction(&self, user_id: &Uuid, message_id: &Uuid, reaction: UserReaction) -> Result<(), ConversationError> {
        let mut context = self.get_or_create_context(user_id).await?;
        
        // Find and update the message
        for message in &mut context.recent_messages {
            if message.id == *message_id {
                message.user_reaction = Some(reaction.clone());
                break;
            }
        }
        
        // Update relationship metrics based on reaction
        self.update_relationship_metrics(&mut context, &reaction).await;
        
        // Save updated context
        self.save_context(&context).await?;
        
        Ok(())
    }

    /// Update relationship metrics based on user reaction
    async fn update_relationship_metrics(&self, context: &mut ConversationContext, reaction: &UserReaction) -> () {
        let metrics = &mut context.relationship_metrics;
        
        // Update engagement level (exponential moving average)
        let alpha = 0.1; // Smoothing factor
        metrics.engagement_level = alpha * reaction.engagement_score + (1.0 - alpha) * metrics.engagement_level;
        
        // Update trust level based on reaction type
        match reaction.reaction_type {
            UserReactionType::Positive | UserReactionType::Curious => {
                metrics.trust_level = (metrics.trust_level + 0.05).min(1.0);
            }
            UserReactionType::Negative | UserReactionType::Dismissive => {
                metrics.trust_level = (metrics.trust_level - 0.02).max(0.0);
            }
            UserReactionType::Neutral => {
                // Slight positive for engagement
                metrics.trust_level = (metrics.trust_level + 0.01).min(1.0);
            }
        }
        
        // Update average response time
        if let Some(response_time) = reaction.response_time_seconds {
            if metrics.avg_response_time == 0.0 {
                metrics.avg_response_time = response_time as f64;
            } else {
                metrics.avg_response_time = alpha * (response_time as f64) + (1.0 - alpha) * metrics.avg_response_time;
            }
        }
    }

    /// Evolve twin personality based on conversation patterns
    pub async fn evolve_personality(&self, user_id: &Uuid) -> Result<Option<PersonalityChange>, ConversationError> {
        let mut context = self.get_or_create_context(user_id).await?;
        
        // Analyze recent conversation patterns
        let recent_engagement = self.analyze_recent_engagement(&context);
        let humor_success = self.analyze_humor_success(&context);
        
        let mut personality_change = None;
        
        // Adjust humor level based on user reactions
        if humor_success < 0.3 && context.twin_personality.humor_level > 3 {
            let old_humor = context.twin_personality.humor_level;
            context.twin_personality.humor_level -= 1;
            
            personality_change = Some(PersonalityChange {
                timestamp: Utc::now(),
                trait_name: "humor_level".to_string(),
                old_value: old_humor,
                new_value: context.twin_personality.humor_level,
                trigger_reason: format!("Low humor success rate: {:.2}", humor_success),
                confidence: 0.8,
            });
        } else if humor_success > 0.8 && context.twin_personality.humor_level < 9 {
            let old_humor = context.twin_personality.humor_level;
            context.twin_personality.humor_level += 1;
            
            personality_change = Some(PersonalityChange {
                timestamp: Utc::now(),
                trait_name: "humor_level".to_string(),
                old_value: old_humor,
                new_value: context.twin_personality.humor_level,
                trigger_reason: format!("High humor success rate: {:.2}", humor_success),
                confidence: 0.8,
            });
        }
        
        // Adjust empathy based on user engagement
        if recent_engagement < 0.4 && context.twin_personality.empathy_level < 9 {
            let old_empathy = context.twin_personality.empathy_level;
            context.twin_personality.empathy_level += 1;
            
            personality_change = Some(PersonalityChange {
                timestamp: Utc::now(),
                trait_name: "empathy_level".to_string(),
                old_value: old_empathy,
                new_value: context.twin_personality.empathy_level,
                trigger_reason: format!("Low engagement, increasing empathy: {:.2}", recent_engagement),
                confidence: 0.7,
            });
        }
        
        // Record personality change
        if let Some(ref change) = personality_change {
            context.twin_personality.personality_evolution_log.push(change.clone());
            
            // Keep only last 20 personality changes
            if context.twin_personality.personality_evolution_log.len() > 20 {
                context.twin_personality.personality_evolution_log.remove(0);
            }
        }
        
        // Save updated context
        self.save_context(&context).await?;
        
        Ok(personality_change)
    }

    /// Analyze recent user engagement patterns
    fn analyze_recent_engagement(&self, context: &ConversationContext) -> f64 {
        let recent_messages: Vec<_> = context.recent_messages
            .iter()
            .rev()
            .take(10) // Last 10 messages
            .filter(|msg| matches!(msg.sender, MessageSender::User))
            .collect();
        
        if recent_messages.is_empty() {
            return 0.5; // Default neutral
        }
        
        let engagement_scores: Vec<f64> = recent_messages
            .iter()
            .filter_map(|msg| msg.user_reaction.as_ref().map(|r| r.engagement_score))
            .collect();
            
        if engagement_scores.is_empty() {
            0.5
        } else {
            engagement_scores.iter().sum::<f64>() / engagement_scores.len() as f64
        }
    }

    /// Analyze humor success rate
    fn analyze_humor_success(&self, context: &ConversationContext) -> f64 {
        let humor_messages: Vec<_> = context.recent_messages
            .iter()
            .filter(|msg| {
                matches!(msg.sender, MessageSender::Twin) && 
                msg.context_tags.contains(&"humor".to_string())
            })
            .collect();
        
        if humor_messages.is_empty() {
            return 0.5; // Default neutral
        }
        
        let positive_reactions = humor_messages
            .iter()
            .filter(|msg| {
                msg.user_reaction
                    .as_ref()
                    .map(|r| matches!(r.reaction_type, UserReactionType::Positive | UserReactionType::Curious))
                    .unwrap_or(false)
            })
            .count();
            
        positive_reactions as f64 / humor_messages.len() as f64
    }

    /// Get conversation statistics
    pub async fn get_conversation_stats(&self, user_id: &Uuid) -> Result<ConversationStats, ConversationError> {
        let context = self.get_or_create_context(user_id).await?;
        
        Ok(ConversationStats {
            total_messages: context.total_messages,
            relationship_stage: context.conversation_summary.relationship_stage.as_str().to_string(),
            engagement_level: context.relationship_metrics.engagement_level,
            trust_level: context.relationship_metrics.trust_level,
            humor_level: context.twin_personality.humor_level,
            empathy_level: context.twin_personality.empathy_level,
            personality_changes: context.twin_personality.personality_evolution_log.len(),
        })
    }

    /// Clean up old conversation data
    pub async fn cleanup_old_conversations(&self, days_to_keep: u64) -> Result<usize, ConversationError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        
        let cutoff_timestamp = (Utc::now() - chrono::Duration::days(days_to_keep as i64)).timestamp() as f64;
        
        // Get all conversation message keys
        let pattern = "conversation:messages:*";
        let keys: Vec<String> = conn.keys(pattern).await?;
        
        let mut cleaned_count = 0;
        
        for key in keys {
            // Remove old messages from sorted set
            let removed: i32 = redis::cmd("ZREMRANGEBYSCORE")
                .arg(&key)
                .arg(0.0)
                .arg(cutoff_timestamp)
                .query_async(&mut conn)
                .await?;

            cleaned_count += removed as usize;
        }
        
        tracing::info!("Cleaned up {} old conversation messages", cleaned_count);
        Ok(cleaned_count)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConversationStats {
    pub total_messages: usize,
    pub relationship_stage: String,
    pub engagement_level: f64,
    pub trust_level: f64,
    pub humor_level: i32,
    pub empathy_level: i32,
    pub personality_changes: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ConversationError {
    #[error("Redis error: {0}")]
    Redis(#[from] RedisError),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Context not found for user: {0}")]
    ContextNotFound(Uuid),
    
    #[error("Invalid message format")]
    InvalidMessageFormat,
}