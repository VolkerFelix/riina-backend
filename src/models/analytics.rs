use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;

/// Analytics event data - structured, validated types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EventData {
    /// Session events (app_session_start, app_session_end)
    Session {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_minutes: Option<i64>,
    },
    /// Screen events (screen_view, screen_exit, screen_enter)
    Screen {
        screen_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_seconds: Option<i64>,
    },
}

/// Analytics event from the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsEvent {
    pub event_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_data: Option<EventData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_hash: Option<String>,
    pub timestamp: i64,
    pub platform: String,
}

/// Request body for batch analytics events
#[derive(Debug, Deserialize)]
pub struct AnalyticsEventsRequest {
    pub events: Vec<AnalyticsEvent>,
}

/// Database model for analytics events
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AnalyticsEventRecord {
    pub id: i64,
    pub event_name: String,
    pub event_data: Option<Json<EventData>>,
    pub screen_name: Option<String>,
    pub session_id: Option<String>,
    pub user_hash: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub platform: String,
    pub created_at: DateTime<Utc>,
}

impl AnalyticsEvent {
    /// Convert timestamp from milliseconds to DateTime
    pub fn get_timestamp(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.timestamp)
            .unwrap_or_else(Utc::now)
    }

    /// Validate event data matches event name expectations
    pub fn validate(&self) -> Result<(), &'static str> {
        match self.event_name.as_str() {
            "app_session_start" | "app_session_end" => {
                if let Some(EventData::Session { .. }) = &self.event_data {
                    Ok(())
                } else {
                    Err("Session events require Session event data")
                }
            }
            "screen_view" | "screen_enter" | "screen_exit" | "screen_time_spent" => {
                if let Some(EventData::Screen { .. }) = &self.event_data {
                    Ok(())
                } else if self.event_data.is_none() {
                    Ok(()) // Screen events can have no data (screen_name is at top level)
                } else {
                    Err("Screen events require Screen event data or no data")
                }
            }
            _ => Ok(()) // Other events can have any data or no data
        }
    }
}
