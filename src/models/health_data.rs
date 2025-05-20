use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Debug, FromRow, Serialize)]
pub struct HealthData {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub steps: Option<i32>,
    #[serde(default)]
    pub heart_rate: Option<f32>,
    #[serde(default)]
    pub sleep: Option<Json<SleepData>>,
    #[serde(default)]
    pub active_energy_burned: Option<f32>,
    #[serde(default)]
    pub additional_metrics: Option<Json<AdditionalMetrics>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SleepData {
    pub total_sleep_hours: f32,
    #[serde(default)]
    pub in_bed_time: Option<i64>,
    #[serde(default)]
    pub out_bed_time: Option<i64>,
    #[serde(default)]
    pub time_in_bed: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdditionalMetrics {
    pub blood_oxygen: Option<i16>,
    pub rest_heart_rate: Option<i16>,
    pub heart_rate_variability: Option<i16>,
    pub respiratory_rate: Option<i16>,
    pub stress_level: Option<i16>,
}

#[derive(Debug, Deserialize)]
pub struct HealthDataSyncRequest {
    pub device_id: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub steps: Option<i32>,
    #[serde(default)]
    pub heart_rate: Option<f32>,
    #[serde(default)]
    pub sleep: Option<SleepData>,
    #[serde(default)]
    pub active_energy_burned: Option<f32>,
    #[serde(default)]
    pub additional_metrics: Option<AdditionalMetrics>,
}

#[derive(Debug, Serialize)]
pub struct HealthDataSyncResponse {
    pub success: bool,
    pub message: String,
    pub sync_id: Uuid,
    pub timestamp: DateTime<Utc>,
}