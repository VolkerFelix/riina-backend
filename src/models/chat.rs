// src/models/chat.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Team chat message model
#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct TeamChatMessage {
    pub id: Uuid,
    pub team_id: Uuid,
    pub user_id: Uuid,
    pub message: String,
    pub reply_to_message_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Team chat message with user information
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamChatMessageInfo {
    pub id: Uuid,
    pub team_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub profile_picture_url: Option<String>,
    pub message: String,
    pub reply_to_message_id: Option<Uuid>,
    pub reply_to_message: Option<String>,
    pub reply_to_username: Option<String>,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Request to send a chat message
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SendChatMessageRequest {
    pub message: String,
    pub reply_to_message_id: Option<Uuid>,
}

/// Request to edit a chat message
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EditChatMessageRequest {
    pub message: String,
}

/// Response for chat message operations
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessageResponse {
    pub success: bool,
    pub message: String,
    pub chat_message: Option<TeamChatMessageInfo>,
}

/// Response for fetching chat history
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatHistoryResponse {
    pub success: bool,
    pub messages: Vec<TeamChatMessageInfo>,
    pub total_count: i64,
    pub has_more: bool,
}

impl SendChatMessageRequest {
    /// Validate send chat message request
    pub fn validate(&self) -> Result<(), String> {
        let trimmed = self.message.trim();

        if trimmed.is_empty() {
            return Err("Message cannot be empty".to_string());
        }

        if trimmed.len() > 5000 {
            return Err("Message cannot exceed 5000 characters".to_string());
        }

        Ok(())
    }

    /// Get sanitized message
    pub fn get_sanitized_message(&self) -> String {
        self.message
            .trim()
            .chars()
            .filter(|&c| c != '\0')
            .collect::<String>()
            .trim()
            .to_string()
    }
}

impl EditChatMessageRequest {
    /// Validate edit chat message request
    pub fn validate(&self) -> Result<(), String> {
        let trimmed = self.message.trim();

        if trimmed.is_empty() {
            return Err("Message cannot be empty".to_string());
        }

        if trimmed.len() > 5000 {
            return Err("Message cannot exceed 5000 characters".to_string());
        }

        Ok(())
    }

    /// Get sanitized message
    pub fn get_sanitized_message(&self) -> String {
        self.message
            .trim()
            .chars()
            .filter(|&c| c != '\0')
            .collect::<String>()
            .trim()
            .to_string()
    }
}
