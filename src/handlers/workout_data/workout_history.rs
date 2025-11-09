use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;
use chrono::{DateTime, Utc, Duration};

use crate::{middleware::auth::Claims, models::workout_data::HeartRateData};

#[derive(Debug, Serialize)]
pub struct WorkoutHistoryItem {
    pub id: Uuid,
    pub workout_date: DateTime<Utc>,
    pub workout_start: DateTime<Utc>,
    pub workout_end: DateTime<Utc>,
    pub duration_minutes: Option<i32>,
    pub calories_burned: Option<i32>,
    pub activity_name: Option<String>,
    pub user_activity: Option<String>,
    pub avg_heart_rate: Option<i32>,
    pub max_heart_rate: Option<i32>,
    pub heart_rate_zones: Option<serde_json::Value>,
    pub heart_rate_data: Option<Vec<HeartRateData>>,
    // Game stats gained from this workout
    pub stamina_gained: f32,
    pub strength_gained: f32,
    // Post information for editing
    pub post_id: Uuid,
    pub post_content: Option<String>,
    pub post_visibility: String,
    pub post_is_editable: bool,
    pub post_created_at: DateTime<Utc>,
    pub post_updated_at: DateTime<Utc>,
    pub post_edited_at: DateTime<Utc>,
    pub post_media_urls: Option<serde_json::Value>,
    // Effort rating
    pub effort_rating: Option<i16>,
    pub needs_effort_rating: bool,
}

#[derive(Debug, Deserialize)]
pub struct WorkoutHistoryQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub include_heart_rate_data: Option<bool>,
    pub user_id: Option<String>,
}

fn calculate_duration_minutes(start: DateTime<Utc>, end: DateTime<Utc>) -> Option<i32> {
    let duration = end.signed_duration_since(start);
    // Only return positive durations
    if duration > Duration::zero() {
        Some((duration.num_seconds() / 60) as i32)
    } else {
        None
    }
}

fn calculate_avg_heart_rate(heart_rate_data: &Vec<HeartRateData>) -> Option<i32> {
    if heart_rate_data.is_empty() {
        return None;
    }
    let sum: i32 = heart_rate_data.iter().map(|hr| hr.heart_rate).sum();
    let count = heart_rate_data.len() as i32;
    Some((sum / count) as i32)
}

fn calculate_max_heart_rate(heart_rate_data: &Vec<HeartRateData>) -> Option<i32> {
    heart_rate_data.iter().map(|hr| hr.heart_rate).reduce(i32::max)
}

#[tracing::instrument(
    name = "Get user workout history",
    skip(pool, claims),
    fields(username = %claims.username)
)]
pub async fn get_workout_history(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    query: web::Query<WorkoutHistoryQuery>,
) -> HttpResponse {
    // Check if a user_id query parameter was provided
    let user_id = if let Some(user_id_str) = &query.user_id {
        // Requesting another user's workout history
        match Uuid::parse_str(user_id_str) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("Failed to parse user_id query parameter: {}", e);
                return HttpResponse::BadRequest().json(json!({
                    "error": "Invalid user_id parameter"
                }));
            }
        }
    } else {
        // Default: get the current user's own workout history
        match Uuid::parse_str(&claims.sub) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("Failed to parse user ID: {}", e);
                return HttpResponse::BadRequest().json(json!({
                    "error": "Invalid user ID"
                }));
            }
        }
    };

    let limit = query.limit.unwrap_or(20).min(100); // Max 100 items
    let offset = query.offset.unwrap_or(0);

    // Fetch workout history as posts that wrap workouts
    let workouts: Vec<WorkoutHistoryItem> = match sqlx::query!(
        r#"
        SELECT
            wd.id,
            COALESCE(wd.workout_start, wd.created_at) as workout_date,
            wd.workout_start,
            wd.workout_end,
            wd.created_at,
            wd.calories_burned as calories_burned,
            wd.duration_minutes,
            wd.activity_name,
            wd.user_activity,
            wd.avg_heart_rate,
            wd.max_heart_rate,
            wd.heart_rate_data,
            wd.heart_rate_zones,
            COALESCE(wd.stamina_gained, 0.0) as stamina_gained,
            COALESCE(wd.strength_gained, 0.0) as strength_gained,
            p.id as post_id,
            p.content,
            p.visibility::text as post_visibility,
            p.is_editable,
            p.created_at as post_created_at,
            COALESCE(p.updated_at, p.created_at) as post_updated_at,
            COALESCE(p.edited_at, p.created_at) as post_edited_at,
            p.media_urls as post_media_urls,
            wsf.effort_rating as "effort_rating?"
        FROM workout_data wd
        INNER JOIN posts p ON p.workout_id = wd.id AND p.user_id = wd.user_id
        LEFT JOIN workout_scoring_feedback wsf ON wsf.workout_data_id = wd.id AND wsf.user_id = wd.user_id
        WHERE wd.user_id = $1
        AND (wd.calories_burned > 100 OR wd.heart_rate_data IS NOT NULL)
        ORDER BY COALESCE(wd.workout_start, wd.created_at) DESC
        LIMIT $2 OFFSET $3
        "#,
        user_id,
        limit as i64,
        offset as i64
    )
    .fetch_all(&**pool)
    .await
    {
        Ok(rows) => {
            rows.into_iter().map(|row| {
                // Use pre-calculated values from database when available
                let duration_minutes = row.duration_minutes
                    .or_else(|| calculate_duration_minutes(row.workout_start, row.workout_end));
                
                // Use pre-calculated heart rate stats or calculate from raw data if needed
                let (avg_heart_rate, max_heart_rate) = if row.avg_heart_rate.is_some() && row.max_heart_rate.is_some() {
                    (row.avg_heart_rate, row.max_heart_rate)
                } else {
                    // Fallback: calculate from raw data if pre-calculated values missing
                    let heart_rate_data: Vec<HeartRateData> = 
                        serde_json::from_value(row.heart_rate_data.clone()).unwrap_or_default();
                    (
                        calculate_avg_heart_rate(&heart_rate_data),
                        calculate_max_heart_rate(&heart_rate_data)
                    )
                };

                // Parse heart rate data from JSON only if requested
                let heart_rate_data = if query.include_heart_rate_data.unwrap_or(false) {
                    if !row.heart_rate_data.is_null() {
                        serde_json::from_value(row.heart_rate_data.clone()).ok()
                    } else {
                        None
                    }
                } else {
                    None
                };

                WorkoutHistoryItem {
                    id: row.id,
                    workout_date: row.workout_date.unwrap_or(row.created_at),
                    workout_start: row.workout_start,
                    workout_end: row.workout_end,
                    duration_minutes,
                    calories_burned: row.calories_burned,
                    activity_name: row.activity_name,
                    user_activity: row.user_activity,
                    avg_heart_rate,
                    max_heart_rate,
                    heart_rate_zones: row.heart_rate_zones,
                    heart_rate_data,
                    stamina_gained: row.stamina_gained.unwrap_or(0.0),
                    strength_gained: row.strength_gained.unwrap_or(0.0),
                    // Post information
                    post_id: row.post_id,
                    post_content: row.content,
                    post_visibility: row.post_visibility.unwrap(),
                    post_is_editable: row.is_editable,
                    post_created_at: row.post_created_at,
                    post_updated_at: row.post_updated_at.unwrap(),
                    post_edited_at: row.post_edited_at.unwrap(),
                    post_media_urls: row.post_media_urls,
                    // Effort rating
                    effort_rating: row.effort_rating,
                    needs_effort_rating: row.effort_rating.is_none(),
                }
            }).collect()
        },
        Err(e) => {
            tracing::error!("Failed to fetch workout history: {}", e);
            return HttpResponse::InternalServerError().json(json!({
                "error": "Failed to fetch workout history"
            }));
        }
    };

    // Get total count for pagination
    let total_count = match sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM workout_data
        WHERE user_id = $1
        AND (calories_burned > 100 OR heart_rate_data IS NOT NULL)
        "#,
        user_id
    )
    .fetch_one(&**pool)
    .await
    {
        Ok(row) => row.count.unwrap_or(0),
        Err(_) => 0,
    };
    

    tracing::info!(
        "Successfully retrieved {} workouts for user: {}",
        workouts.len(),
        claims.username
    );

    HttpResponse::Ok().json(json!({
        "success": true,
        "data": {
            "workouts": workouts,
            "pagination": {
                "total": total_count,
                "limit": limit,
                "offset": offset,
                "has_more": (offset as i64 + limit as i64) < total_count
            }
        }
    }))
}