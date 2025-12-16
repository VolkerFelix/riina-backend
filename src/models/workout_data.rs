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
    pub activity_name: Option<String>,  // Original activity from health data source (read-only)
    pub user_activity: Option<String>,  // User-edited activity type (takes precedence)
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

#[derive(PartialEq, Clone)]
pub enum WorkoutType {
    Strength,
    Cardio,
    Hiit,
    Other
}

impl WorkoutType {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkoutType::Strength => "strength",
            WorkoutType::Cardio => "cardio",
            WorkoutType::Hiit => "hiit",
            WorkoutType::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "strength" => WorkoutType::Strength,
            "cardio" => WorkoutType::Cardio,
            "hiit" => WorkoutType::Hiit,
            "other" => WorkoutType::Other,
            _ => WorkoutType::Other,
        }
    }
}


#[derive(Debug, FromRow, Serialize)]
pub struct WorkoutScoringFeedback {
    pub id: Uuid,
    pub workout_data_id: Uuid,
    pub user_id: Uuid,
    pub effort_rating: i16,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitScoringFeedbackRequest {
    pub effort_rating: i16,
}

impl SubmitScoringFeedbackRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.effort_rating < 0 || self.effort_rating > 10 {
            return Err("Effort rating must be between 0 and 10".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, FromRow, Serialize)]
pub struct WorkoutReport {
    pub id: Uuid,
    pub workout_data_id: Uuid,
    pub reported_by_user_id: Uuid,
    pub workout_owner_id: Uuid,
    pub reason: String,
    pub status: String,
    pub admin_notes: Option<String>,
    pub reviewed_by_user_id: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitWorkoutReportRequest {
    pub reason: String,
}

impl SubmitWorkoutReportRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.reason.trim().is_empty() {
            return Err("Reason cannot be empty".to_string());
        }
        if self.reason.len() > 1000 {
            return Err("Reason must be 1000 characters or less".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkoutReportRequest {
    pub status: String,
    pub admin_notes: Option<String>,
}

impl UpdateWorkoutReportRequest {
    pub fn validate(&self) -> Result<(), String> {
        if !["pending", "reviewed", "dismissed", "confirmed"].contains(&self.status.as_str()) {
            return Err("Invalid status".to_string());
        }
        if let Some(notes) = &self.admin_notes {
            if notes.len() > 2000 {
                return Err("Admin notes must be 2000 characters or less".to_string());
            }
        }
        Ok(())
    }
}