use sqlx::{Pool, Postgres};
use uuid::Uuid;
use serde_json::json;
use chrono::Duration;

use crate::models::workout_data::{WorkoutDataSyncRequest, HeartRateData};

/// Calculate duration in minutes from start/end times
fn calculate_duration_minutes(data: &WorkoutDataSyncRequest) -> Option<i32> {
    match (&data.workout_start, &data.workout_end) {
        (Some(start), Some(end)) => {
            let duration = end.signed_duration_since(*start);
            if duration > Duration::zero() {
                Some((duration.num_seconds() / 60) as i32)
            } else {
                None
            }
        }
        _ => None
    }
}

/// Calculate average heart rate from heart rate data
fn calculate_avg_heart_rate(heart_rate_data: &[HeartRateData]) -> Option<i32> {
    if heart_rate_data.is_empty() {
        return None;
    }
    
    let sum: i32 = heart_rate_data.iter().map(|hr| hr.heart_rate).sum();
    Some(sum / heart_rate_data.len() as i32)
}

/// Calculate maximum heart rate from heart rate data
fn calculate_max_heart_rate(heart_rate_data: &[HeartRateData]) -> Option<i32> {
    heart_rate_data.iter().map(|hr| hr.heart_rate).reduce(i32::max)
}

/// Calculate minimum heart rate from heart rate data  
fn calculate_min_heart_rate(heart_rate_data: &[HeartRateData]) -> Option<i32> {
    heart_rate_data.iter().map(|hr| hr.heart_rate).reduce(i32::min)
}

#[tracing::instrument(
    name = "Insert workout data into database",
    skip(pool, data),
    fields(
        user_id = %user_id,
        workout_uuid = ?data.workout_uuid,
        device_id = %data.device_id,
        is_duplicate = %is_duplicate
    )
)]
pub async fn insert_workout_data(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    data: &WorkoutDataSyncRequest,
    is_duplicate: bool,
) -> Result<Uuid, sqlx::Error> {
    tracing::info!("Attempting to insert workout data for user (is_duplicate: {})", is_duplicate);
    
    // Calculate derived metrics
    let duration_minutes = calculate_duration_minutes(data);
    
    let (avg_heart_rate, max_heart_rate, min_heart_rate) = if let Some(heart_rate_data) = &data.heart_rate {
        (
            calculate_avg_heart_rate(heart_rate_data),
            calculate_max_heart_rate(heart_rate_data),
            calculate_min_heart_rate(heart_rate_data),
        )
    } else {
        (None, None, None)
    };
    
    let record = sqlx::query!(
        r#"
        INSERT INTO workout_data (
            user_id, device_id, heart_rate_data, 
            calories_burned, workout_uuid, workout_start, workout_end,
            duration_minutes, avg_heart_rate, max_heart_rate, min_heart_rate,
            is_duplicate
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING id
        "#,
        user_id,
        &data.device_id,
        json!(data.heart_rate),
        data.calories_burned,
        data.workout_uuid,
        data.workout_start,
        data.workout_end,
        duration_minutes,
        avg_heart_rate,
        max_heart_rate,
        min_heart_rate,
        is_duplicate
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        // Check if this is a unique constraint violation
        if let sqlx::Error::Database(ref db_err) = e {
            // PostgreSQL unique constraint violation error code is 23505
            if db_err.code().as_deref() == Some("23505") {
                tracing::error!(
                    "DUPLICATE WORKOUT UUID ERROR: Failed to insert workout data due to duplicate workout_uuid. \
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
                    "Database error inserting workout data: code={:?}, message={}, \
                    constraint={:?}, detail={:?}",
                    db_err.code(),
                    db_err.message(),
                    db_err.constraint(),
                    db_err.message()
                );
            }
        } else {
            tracing::error!("Non-database error inserting workout data: {}", e);
        }
        e
    })?;
    
    tracing::info!("Successfully inserted workout data with id: {}", record.id);
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
        SELECT id FROM workout_data 
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

#[tracing::instrument(
    name = "Check for duplicate workout by time",
    skip(pool, data),
    fields(
        user_id = %user_id,
        workout_start = ?data.workout_start,
        workout_end = ?data.workout_end
    )
)]
pub async fn check_duplicate_workout_by_time(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    data: &WorkoutDataSyncRequest,
) -> Result<bool, sqlx::Error> {
    // Only check if we have both start and end times
    let (workout_start, workout_end) = match (&data.workout_start, &data.workout_end) {
        (Some(start), Some(end)) => (start, end),
        _ => {
            tracing::debug!("No workout start/end times provided, skipping time-based duplicate check");
            return Ok(false);
        }
    };
    
    tracing::info!(
        "Checking for duplicate workouts with time tolerance against non-duplicate workouts only. Start: {} ± 15s, End: {} ± 15s",
        workout_start, workout_end
    );
    
    // Check if there's any workout with overlapping time windows
    // We need to check if there's an existing workout where:
    // - The new workout's start time is within ±15s of existing workout's start time
    // - AND the new workout's end time is within ±15s of existing workout's end time  
    let record = sqlx::query!(
        r#"
        SELECT id, workout_uuid, workout_start, workout_end
        FROM workout_data 
        WHERE user_id = $1
        AND workout_start IS NOT NULL
        AND workout_end IS NOT NULL
        AND is_duplicate = false
        AND ABS(EXTRACT(EPOCH FROM (workout_start - $2))) <= 15
        AND ABS(EXTRACT(EPOCH FROM (workout_end - $3))) <= 15
        AND workout_uuid != $4
        LIMIT 1
        "#,
        user_id,
        workout_start,
        workout_end,
        &data.workout_uuid
    )
    .fetch_optional(pool)
    .await?;
    
    if let Some(rec) = record {
        tracing::warn!(
            "Found potential duplicate workout! Existing workout id: {}, uuid: {:?}, \
            start: {:?}, end: {:?}. New workout uuid: {:?}, start: {:?}, end: {:?}",
            rec.id, rec.workout_uuid, rec.workout_start, rec.workout_end,
            data.workout_uuid, workout_start, workout_end
        );
        Ok(true)
    } else {
        tracing::debug!("No duplicate workout found based on time check");
        Ok(false)
    }
}