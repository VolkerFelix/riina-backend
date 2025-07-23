use sqlx::{Pool, Postgres};
use uuid::Uuid;
use serde_json::json;

use crate::models::health_data::HealthDataSyncRequest;

#[tracing::instrument(
    name = "Insert health data into database",
    skip(pool, data),
    fields(
        user_id = %user_id,
        workout_uuid = ?data.workout_uuid,
        device_id = %data.device_id
    )
)]
pub async fn insert_health_data(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    data: &HealthDataSyncRequest,
) -> Result<Uuid, sqlx::Error> {
    tracing::info!("Attempting to insert health data for user");
    
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
    .await
    .map_err(|e| {
        // Check if this is a unique constraint violation
        if let sqlx::Error::Database(ref db_err) = e {
            // PostgreSQL unique constraint violation error code is 23505
            if db_err.code().as_deref() == Some("23505") {
                tracing::error!(
                    "DUPLICATE WORKOUT UUID ERROR: Failed to insert health data due to duplicate workout_uuid. \
                    user_id: {}, workout_uuid: {:?}, device_id: {}, \
                    constraint: {}, detail: {:?}, table: {:?}",
                    user_id,
                    data.workout_uuid,
                    data.device_id,
                    db_err.constraint().unwrap_or("unknown"),
                    db_err.message(),
                    db_err.table()
                );
            } else {
                tracing::error!(
                    "Database error inserting health data: code={:?}, message={}, \
                    constraint={:?}, detail={:?}",
                    db_err.code(),
                    db_err.message(),
                    db_err.constraint(),
                    db_err.message()
                );
            }
        } else {
            tracing::error!("Non-database error inserting health data: {}", e);
        }
        e
    })?;
    
    tracing::info!("Successfully inserted health data with id: {}", record.id);
    Ok(record.id)
}

#[tracing::instrument(
    name = "Check if workout UUID exists",
    skip(pool),
    fields(
        user_id = %user_id,
        workout_uuid = %workout_uuid
    )
)]
pub async fn check_workout_uuid_exists(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    workout_uuid: &str,
) -> Result<bool, sqlx::Error> {
    tracing::debug!("Checking if workout UUID exists for user");
    
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
    
    let exists = record.is_some();
    tracing::info!("Workout UUID check result: exists={}", exists);
    
    Ok(exists)
}