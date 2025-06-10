use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct UserProfileResponse {
    pub id: Uuid,
    pub username: String,
    pub level: i32,
    pub experience_points: i64,
    pub stats: GameStats,
    pub rank: i32,
    pub avatar_style: String,
    pub total_stats: i32,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
}

#[derive(serde::Serialize)]
pub struct GameStats {
    pub stamina: i32,
    pub strength: i32,
    pub experience_points: i64,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct HealthProfileResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub age: Option<i32>,
    pub gender: Option<String>,
    pub resting_heart_rate: Option<i32>,
    pub weight: Option<f32>,
    pub height: Option<f32>,
    pub last_updated: DateTime<Utc>,
}

#[derive(serde::Deserialize)]
pub struct UpdateHealthProfileRequest {
    pub age: Option<i32>,
    pub gender: Option<String>,
    pub resting_heart_rate: Option<i32>,
    pub weight: Option<f32>,
    pub height: Option<f32>,
}