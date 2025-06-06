use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow, Serialize)]
pub struct HealthData {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: String,
    pub timestamp: DateTime<Utc>,
    pub heart_rate: Option<Vec<HeartRateData>>,
    pub active_energy_burned: Option<f32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HeartRateData {
    pub timestamp: DateTime<Utc>,
    pub heart_rate: f32,
}

#[derive(Debug, Deserialize)]
pub struct HealthDataSyncRequest {
    pub device_id: String,
    pub timestamp: DateTime<Utc>,
    pub heart_rate: Option<Vec<HeartRateData>>,
    pub active_energy_burned: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct HealthDataSyncResponse {
    pub success: bool,
    pub message: String,
    pub sync_id: Uuid,
    pub timestamp: DateTime<Utc>,
}