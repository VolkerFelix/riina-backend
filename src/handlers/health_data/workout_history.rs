use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;
use chrono::{DateTime, Utc, Duration};

use crate::{middleware::auth::Claims, models::health_data::HeartRateData};

#[derive(Debug, Serialize)]
pub struct WorkoutHistoryItem {
    pub id: Uuid,
    pub workout_date: DateTime<Utc>,
    pub workout_start: Option<DateTime<Utc>>,
    pub workout_end: Option<DateTime<Utc>>,
    pub duration_minutes: Option<i32>,
    pub calories_burned: Option<f32>,
    pub avg_heart_rate: Option<f32>,
    pub max_heart_rate: Option<f32>,
    pub heart_rate_zones: Option<serde_json::Value>,
    // Game stats gained from this workout
    pub stamina_gained: i32,
    pub strength_gained: i32,
}

#[derive(Debug, Deserialize)]
pub struct WorkoutHistoryQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

fn calculate_duration_minutes(start: Option<DateTime<Utc>>, end: Option<DateTime<Utc>>) -> Option<i32> {
    match (start, end) {
        (Some(s), Some(e)) => {
            let duration = e.signed_duration_since(s);
            // Only return positive durations
            if duration > Duration::zero() {
                Some((duration.num_seconds() / 60) as i32)
            } else {
                None
            }
        }
        _ => None
    }
}

fn calculate_avg_heart_rate(heart_rate_data: &Vec<HeartRateData>) -> Option<f32> {
    if heart_rate_data.is_empty() {
        return None;
    }
    let sum: f32 = heart_rate_data.iter().map(|hr| hr.heart_rate).sum();
    let count = heart_rate_data.len() as f32;
    Some(sum / count)
}

fn calculate_max_heart_rate(heart_rate_data: &Vec<HeartRateData>) -> Option<f32> {
    heart_rate_data.iter().map(|hr| hr.heart_rate).reduce(f32::max)
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
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "error": "Invalid user ID"
            }));
        }
    };

    let limit = query.limit.unwrap_or(20).min(100); // Max 100 items
    let offset = query.offset.unwrap_or(0);

    // Fetch workout history with game stats and zone breakdown
    let workouts: Vec<WorkoutHistoryItem> = match sqlx::query!(
        r#"
        SELECT 
            hd.id,
            COALESCE(hd.workout_start, hd.created_at) as workout_date,
            hd.workout_start,
            hd.workout_end,
            hd.created_at,
            hd.active_energy_burned as calories_burned,
            hd.heart_rate_data,
            -- Get game stats from stat_changes table
            COALESCE(sc.stamina_change, 0) as stamina_gained,
            COALESCE(sc.strength_change, 0) as strength_gained,
            sc.zone_breakdown
        FROM health_data hd
        LEFT JOIN stat_changes sc ON sc.health_data_id = hd.id
        WHERE hd.user_id = $1
        AND (hd.active_energy_burned > 100 OR hd.heart_rate_data IS NOT NULL)
        ORDER BY COALESCE(hd.workout_start, hd.created_at) DESC
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
                let duration_minutes = calculate_duration_minutes(row.workout_start, row.workout_end);
                
                // Parse heart rate data from JSON (JSONB NOT NULL so always present)
                let heart_rate_data: Vec<HeartRateData> = 
                    serde_json::from_value(row.heart_rate_data.clone()).unwrap_or_default();
                
                let avg_heart_rate = calculate_avg_heart_rate(&heart_rate_data);
                let max_heart_rate = calculate_max_heart_rate(&heart_rate_data);
                let stamina = row.stamina_gained.unwrap_or(0) as i32;
                let strength = row.strength_gained.unwrap_or(0) as i32;
                
                WorkoutHistoryItem {
                    id: row.id,
                    workout_date: row.workout_date.unwrap_or(row.created_at),
                    workout_start: row.workout_start,
                    workout_end: row.workout_end,
                    duration_minutes,
                    calories_burned: row.calories_burned,
                    avg_heart_rate,
                    max_heart_rate,
                    heart_rate_zones: row.zone_breakdown, // Include zone breakdown from stat_changes
                    stamina_gained: stamina,
                    strength_gained: strength,
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
        FROM health_data
        WHERE user_id = $1
        AND (active_energy_burned > 100 OR heart_rate_data IS NOT NULL)
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