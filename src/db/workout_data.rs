use sqlx::{Pool, Postgres};
use uuid::Uuid;
use chrono::{Duration, DateTime, Utc};

use crate::{
    services::ml_client::ClassifyResponse,
    models::workout_data::{HeartRateData, WorkoutDataUploadRequest, WorkoutStats, ZoneBreakdown}
};

/// Calculate duration in minutes from start/end times
fn calculate_duration_minutes(data: &WorkoutDataUploadRequest) -> Option<i32> {
    let duration = data.workout_end.signed_duration_since(data.workout_start);
        if duration > Duration::zero() {
            Some((duration.num_seconds() / 60) as i32)
        } else {
            None
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
        device_id = %data.device_id
    )
)]
pub async fn insert_workout_data(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    data: &WorkoutDataUploadRequest,
    workout_stats: &WorkoutStats,
) -> Result<Uuid, sqlx::Error> {
    tracing::info!("Attempting to insert workout data for user");
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

    let zone_breakdown_json = workout_stats.zone_breakdown.as_ref()
        .map(|breakdown| serde_json::to_value(breakdown).unwrap_or(serde_json::Value::Null));
    
    let record = sqlx::query!(
        r#"
        INSERT INTO workout_data (
            user_id,
            device_id,
            heart_rate_data,
            calories_burned,
            workout_uuid,
            workout_start,
            workout_end,
            duration_minutes,
            avg_heart_rate,
            max_heart_rate,
            min_heart_rate,
            heart_rate_zones,
            stamina_gained,
            strength_gained,
            total_points_gained,
            activity_name,
            visibility
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
        RETURNING id
        "#,
        user_id,
        &data.device_id,
        serde_json::to_value(&data.heart_rate).unwrap_or(serde_json::Value::Null),
        data.calories_burned.unwrap_or(0),
        data.workout_uuid,
        data.workout_start,
        data.workout_end,
        duration_minutes,
        avg_heart_rate,
        max_heart_rate,
        min_heart_rate,
        zone_breakdown_json,
        workout_stats.changes.stamina_change,
        workout_stats.changes.strength_change,
        (workout_stats.changes.stamina_change + workout_stats.changes.strength_change) as i32,
        data.activity_name.as_deref(),
        "public"  // Default visibility for all workouts
    )
    .fetch_one(pool)
    .await?;
    
    tracing::info!("Successfully inserted workout data with id: {}", record.id);
    Ok(record.id)
}

/// Check if a workout exists within the time tolerance window
/// 
/// This function checks if there's an existing workout for the user where both:
/// - The start time is within ±WORKOUT_TIME_TOLERANCE_SECONDS of the provided start time
/// - The end time is within ±WORKOUT_TIME_TOLERANCE_SECONDS of the provided end time
#[tracing::instrument(
    name = "Check workout exists by time",
    skip(pool),
    fields(
        user_id = %user_id,
        workout_start = %workout_start,
        workout_end = %workout_end
    )
)]
pub async fn check_workout_exists_by_time(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    workout_start: &DateTime<Utc>,
    workout_end: &DateTime<Utc>,
    time_tolerance: Duration,
) -> Result<bool, sqlx::Error> {
    let time_tolerance_seconds = time_tolerance.num_seconds() as f64;

    let record = sqlx::query!(
        r#"
        SELECT id
        FROM workout_data 
        WHERE user_id = $1
        AND workout_start IS NOT NULL
        AND workout_end IS NOT NULL
        AND ABS(EXTRACT(EPOCH FROM (workout_start - $2))::float8) <= $4
        AND ABS(EXTRACT(EPOCH FROM (workout_end - $3))::float8) <= $4
        LIMIT 1
        "#,
        user_id,
        workout_start,
        workout_end,
        time_tolerance_seconds
    )
    .fetch_optional(pool)
    .await?;
    
    Ok(record.is_some())
}

#[tracing::instrument(
    name = "Create post for workout",
    skip(pool, user_id, workout_id, image_urls, video_urls, workout_start),
    fields(
        user_id = %user_id,
        workout_id = %workout_id,
        image_urls = ?image_urls,
        video_urls = ?video_urls,
        workout_start = %workout_start
    )
)]
pub async fn create_post_for_workout(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    workout_id: Uuid,
    image_urls: &Option<Vec<String>>,
    video_urls: &Option<Vec<String>>,
    workout_start: DateTime<Utc>,
) -> Result<Uuid, sqlx::Error> {

    // Build media_urls JSONB array from image_urls and video_urls
    let mut media_items = Vec::new();

    if let Some(images) = image_urls {
        for url in images {
            media_items.push(serde_json::json!({"type": "image", "url": url}));
        }
    }

    if let Some(videos) = video_urls {
        for url in videos {
            media_items.push(serde_json::json!({"type": "video", "url": url}));
        }
    }

    let media_urls_json = if media_items.is_empty() {
        None
    } else {
        Some(serde_json::Value::Array(media_items))
    };

    // Create a post for this workout with media files
    let record = sqlx::query!(
        r#"
        INSERT INTO posts (id, user_id, post_type, workout_id, media_urls, visibility, is_editable, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, 'workout'::post_type, $2, $3, 'public'::post_visibility, true, $4, $4)
        RETURNING id
        "#,
        user_id,
        workout_id,
        media_urls_json as Option<serde_json::Value>,
        workout_start
    )
    .fetch_one(pool)
    .await?;

    Ok(record.id)
}

#[tracing::instrument(
    name = "Update workout data with classification and score",
    skip(pool, workout_id, workout_stats, zone_breakdown, ml_classification),
    fields(
        workout_id = %workout_id,
        workout_stats = ?workout_stats,
        zone_breakdown = ?zone_breakdown,
        ml_classification = ?ml_classification
    )
)]
pub async fn update_workout_data_with_classification_and_score(
    pool: &Pool<Postgres>,
    workout_id: Uuid,
    workout_stats: &WorkoutStats,
    zone_breakdown: &Vec<ZoneBreakdown>,
    ml_classification: &ClassifyResponse,
) -> Result<(), sqlx::Error> {
    let ml_prediction = &ml_classification.prediction;
    let ml_confidence = ml_classification.confidence as f32;
    let ml_classified_at = Utc::now();

    sqlx::query!(
            r#"
            UPDATE workout_data
            SET stamina_gained = $1,
                strength_gained = $2,
                total_points_gained = $3,
                heart_rate_zones = $4,
                ml_prediction = $5,
                ml_confidence = $6,
                ml_classified_at = $7
            WHERE id = $8
            "#,
            workout_stats.changes.stamina_change,
            workout_stats.changes.strength_change,
            (workout_stats.changes.stamina_change + workout_stats.changes.strength_change) as i32,
            serde_json::to_value(&zone_breakdown).unwrap_or(serde_json::Value::Null),
            ml_prediction,
            ml_confidence,
            ml_classified_at,
            workout_id
        ).execute(pool)
        .await?;

    Ok(())
}