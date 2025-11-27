use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct UserProfileResponse {
    pub id: Uuid,
    pub username: String,
    pub stats: GameStats,
    pub rank: i32,
    pub avatar_style: String,
    pub total_stats: f32,
    pub trailing_average: f32,
    pub profile_picture_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    #[sqlx(skip)]
    pub mvp_count: i64,
    #[sqlx(skip)]
    pub lvp_count: i64,
    #[sqlx(skip)]
    pub avg_exercise_minutes_per_day: f32,
    #[sqlx(skip)]
    pub team_id: Option<Uuid>,
    #[sqlx(skip)]
    pub team_status: Option<String>,
}

#[derive(serde::Serialize)]
pub struct GameStats {
    pub stamina: f32,
    pub strength: f32,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct HealthProfileResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub age: Option<i32>,
    pub gender: Option<String>,
    pub resting_heart_rate: Option<i32>,
    pub max_heart_rate: Option<i32>,
    pub vt_off_threshold: Option<i32>,
    pub vt0_threshold: Option<i32>,
    pub vt1_threshold: Option<i32>,
    pub vt2_threshold: Option<i32>,
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