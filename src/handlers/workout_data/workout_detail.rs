use actix_web::{web, HttpResponse};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;
use chrono::{DateTime, Utc};

use crate::{middleware::auth::Claims, models::workout_data::HeartRateData};

#[derive(Debug, Serialize)]
pub struct WorkoutDetail {
    pub id: Uuid,
    pub workout_date: Option<DateTime<Utc>>,
    pub workout_start: DateTime<Utc>,
    pub workout_end: DateTime<Utc>,
    pub duration_minutes: Option<i32>,
    pub calories_burned: Option<i32>,
    pub activity_name: Option<String>,
    pub avg_heart_rate: Option<i32>,
    pub max_heart_rate: Option<i32>,
    pub heart_rate_zones: Option<serde_json::Value>,
    pub heart_rate_data: Option<Vec<HeartRateData>>,
    // Game stats gained from this workout
    pub stamina_gained: Option<f32>,
    pub strength_gained: Option<f32>,
    // Media attachments
    pub image_url: Option<String>,
    pub video_url: Option<String>,
}

fn calculate_duration_minutes(start: DateTime<Utc>, end: DateTime<Utc>) -> Option<i32> {
    let duration = end.signed_duration_since(start);
    // Only return positive durations
    if duration > chrono::Duration::zero() {
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
    name = "Get user workout detail",
    skip(pool, claims),
    fields(username = %claims.username, workout_id = %workout_id)
)]
pub async fn get_workout_detail(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    workout_id: web::Path<Uuid>,
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "success": false,
                "error": "Invalid user ID"
            }));
        }
    };

    let workout_id = workout_id.into_inner();

    // Fetch specific workout with all stats from workout_data
    let workout = match sqlx::query!(
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
            wd.avg_heart_rate,
            wd.max_heart_rate,
            wd.heart_rate_data,
            wd.heart_rate_zones,
            COALESCE(wd.stamina_gained, 0.0) as stamina_gained,
            COALESCE(wd.strength_gained, 0.0) as strength_gained,
            wd.image_url,
            wd.video_url
        FROM workout_data wd
        WHERE wd.id = $1
        "#,
        workout_id
    )
    .fetch_optional(&**pool)
    .await
    {
        Ok(Some(row)) => {
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

            // Parse heart rate data from JSON
            let heart_rate_data = if !row.heart_rate_data.is_null() {
                serde_json::from_value(row.heart_rate_data.clone()).ok()
            } else {
                None
            };

            WorkoutDetail {
                id: row.id,
                workout_date: row.workout_date,
                workout_start: row.workout_start,
                workout_end: row.workout_end,
                duration_minutes,
                calories_burned: row.calories_burned,
                activity_name: row.activity_name,
                avg_heart_rate,
                max_heart_rate,
                heart_rate_zones: row.heart_rate_zones,
                heart_rate_data,
                stamina_gained: row.stamina_gained,
                strength_gained: row.strength_gained,
                image_url: row.image_url,
                video_url: row.video_url,
            }
        }
        Ok(None) => {
            return HttpResponse::NotFound().json(json!({
                "success": false,
                "error": "Workout not found"
            }));
        }
        Err(e) => {
            tracing::error!("Database error fetching workout: {}", e);
            return HttpResponse::InternalServerError().json(json!({
                "success": false,
                "error": "Database error"
            }));
        }
    };

    HttpResponse::Ok().json(json!({
        "success": true,
        "data": workout
    }))
}
