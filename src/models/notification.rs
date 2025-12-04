use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct PushToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub platform: String,
    pub device_info: Option<serde_json::Value>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterPushTokenRequest {
    pub token: String,
    pub platform: String,
    pub device_info: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UnregisterPushTokenRequest {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct PushTokenResponse {
    pub id: Uuid,
    pub token: String,
    pub platform: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl From<PushToken> for PushTokenResponse {
    fn from(token: PushToken) -> Self {
        PushTokenResponse {
            id: token.id,
            token: token.token,
            platform: token.platform,
            is_active: token.is_active,
            created_at: token.created_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendNotificationRequest {
    pub user_ids: Vec<Uuid>,
    pub title: String,
    pub body: String,
    pub data: Option<serde_json::Value>,
    pub notification_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SendNotificationResponse {
    pub success: bool,
    pub sent_count: usize,
    pub failed_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExpoPushMessage {
    pub to: String,
    pub title: String,
    pub body: String,
    pub data: Option<serde_json::Value>,
    pub sound: Option<String>,
    pub badge: Option<i32>,
    pub channel_id: Option<String>,
    pub priority: Option<String>,
}

impl ExpoPushMessage {
    pub fn new(token: String, title: String, body: String) -> Self {
        Self {
            to: token,
            title,
            body,
            data: None,
            sound: Some("default".to_string()),
            badge: None,
            channel_id: Some("default".to_string()),
            priority: Some("high".to_string()),
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    pub fn with_channel(mut self, channel_id: String) -> Self {
        self.channel_id = Some(channel_id);
        self
    }

    pub fn with_badge(mut self, badge: i32) -> Self {
        self.badge = Some(badge);
        self
    }
}
