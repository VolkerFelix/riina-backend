use sqlx::{Pool, Postgres};
use sqlx::types::Json;
use uuid::Uuid;

use crate::models::health_data::{HealthData, HealthDataSyncRequest, SleepData};

pub async fn insert_health_data(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    data: &HealthDataSyncRequest,
) -> Result<Uuid, sqlx::Error> {
    // Convert SleepData to Json<SleepData> if present
    let sleep_json = data.sleep.as_ref().map(|s| Json(s.clone()));
    
    // Convert additional_metrics to Json if present
    let additional_metrics_json = data.additional_metrics.as_ref().map(|m| Json(m.clone()));
    
    let record = sqlx::query_as!(
        HealthData,
        r#"
        INSERT INTO health_data (
            user_id, device_id, timestamp, steps, heart_rate, 
            sleep, active_energy_burned, additional_metrics
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id, user_id, device_id, timestamp, steps, heart_rate, 
                  sleep as "sleep: Json<SleepData>", active_energy_burned,
                  additional_metrics as "additional_metrics: Json<serde_json::Value>", created_at
        "#,
        user_id,
        data.device_id,
        data.timestamp,
        data.steps,
        data.heart_rate,
        sleep_json as Option<Json<SleepData>>,
        data.active_energy_burned,
        additional_metrics_json as Option<Json<serde_json::Value>>
    )
    .fetch_one(pool)
    .await?;
    
    Ok(record.id)
}

// This function is used during user registration or login
pub async fn get_user_by_email(
    pool: &Pool<Postgres>,
    email: &str,
) -> Result<Option<crate::models::User>, sqlx::Error> {
    let user = sqlx::query_as!(
        crate::models::User,
        r#"
        SELECT id, email, username, password_hash, created_at, updated_at
        FROM users
        WHERE email = $1
        "#,
        email
    )
    .fetch_optional(pool)
    .await?;
    
    Ok(user)
}

// This function is used during user registration
pub async fn create_user(
    pool: &Pool<Postgres>,
    email: &str,
    username: &str,
    password_hash: &str,
) -> Result<crate::models::User, sqlx::Error> {
    let user = sqlx::query_as!(
        crate::models::User,
        r#"
        INSERT INTO users (email, username, password_hash)
        VALUES ($1, $2, $3)
        RETURNING id, email, username, password_hash, created_at, updated_at
        "#,
        email,
        username,
        password_hash
    )
    .fetch_one(pool)
    .await?;
    
    Ok(user)
}