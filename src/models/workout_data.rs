use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow, Serialize)]
pub struct WorkoutData {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: String,
    pub timestamp: DateTime<Utc>,
    pub heart_rate: Option<Vec<HeartRateData>>,
    pub calories_burned: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub activity_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow, sqlx::Decode)]
pub struct HeartRateData {
    pub timestamp: DateTime<Utc>,
    pub heart_rate: i32,
}

#[derive(Debug, Deserialize)]
pub struct WorkoutDataUploadRequest {
    pub device_id: String,
    pub timestamp: DateTime<Utc>,
    pub heart_rate: Option<Vec<HeartRateData>>,
    pub calories_burned: Option<i32>,
    pub workout_uuid: String,
    pub workout_start: DateTime<Utc>,
    pub workout_end: DateTime<Utc>,
    pub activity_name: Option<String>,
    pub image_urls: Option<Vec<String>>,
    pub video_urls: Option<Vec<String>>,
    pub approval_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WorkoutDataSyncData {
    pub sync_id: Uuid,
    pub timestamp: DateTime<Utc>,
}

/// Response for successful workout upload
#[derive(Debug, Serialize)]
pub struct WorkoutUploadResponse {
    pub sync_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub game_stats: StatChanges,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct StatChanges {
    pub stamina_change: f32,
    pub strength_change: f32,
}

impl StatChanges {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZoneBreakdown {
    pub zone: String,
    pub minutes: f32,
    pub stamina_gained: f32,
    pub strength_gained: f32,
    pub hr_min: Option<i32>, // Lower heart rate limit for this zone
    pub hr_max: Option<i32>, // Upper heart rate limit for this zone
}

impl ZoneBreakdown {
    pub fn new(zone: String) -> Self {
        Self {
            zone,
            minutes: 0.0,
            stamina_gained: 0.0,
            strength_gained: 0.0,
            hr_min: None,
            hr_max: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkoutStats {
    pub changes: StatChanges,
    pub zone_breakdown: Option<Vec<ZoneBreakdown>>,
}

impl WorkoutStats {
    pub fn new() -> Self {
        Self {
            changes: StatChanges::new(),
            zone_breakdown: None,
        }
    }
}