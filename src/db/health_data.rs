use sqlx::{Pool, Postgres};
use uuid::Uuid;
use serde_json::json;

use crate::models::health_data::HealthDataSyncRequest;

pub async fn insert_health_data(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    data: &HealthDataSyncRequest,
) -> Result<Uuid, sqlx::Error> {
    let record = sqlx::query!(
        r#"
        INSERT INTO health_data (
            user_id, device_id, heart_rate_data, 
            active_energy_burned, workout_uuid, workout_start, workout_end
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id
        "#,
        user_id,
        &data.device_id,
        json!(data.heart_rate),
        data.active_energy_burned,
        data.workout_uuid.as_deref(),
        data.workout_start,
        data.workout_end
    )
    .fetch_one(pool)
    .await?;
    
    Ok(record.id)
}

pub async fn check_workout_uuid_exists(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    workout_uuid: &str,
) -> Result<bool, sqlx::Error> {
    let record = sqlx::query!(
        r#"
        SELECT id FROM health_data 
        WHERE user_id = $1 AND workout_uuid = $2
        "#,
        user_id,
        workout_uuid
    )
    .fetch_optional(pool)
    .await?;
    
    Ok(record.is_some())
}