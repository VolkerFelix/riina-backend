use actix_web::{web, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::models::workout_data::HeartRateData;

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AdminWorkoutData {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub device_id: String,
    pub heart_rate_count: i32,
    pub calories_burned: Option<i32>,
    pub workout_uuid: Option<String>,
    pub workout_start: Option<DateTime<Utc>>,
    pub workout_end: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AdminWorkoutDetail {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub device_id: String,
    pub heart_rate: Option<Vec<HeartRateData>>,
    pub calories_burned: Option<i32>,
    pub workout_uuid: Option<String>,
    pub workout_start: Option<DateTime<Utc>>,
    pub workout_end: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AdminWorkoutQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub user_id: Option<Uuid>,
    pub username: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AdminWorkoutResponse {
    pub workouts: Vec<AdminWorkoutData>,
    pub total: i64,
}

pub async fn get_all_workouts(
    pool: web::Data<PgPool>,
    query: web::Query<AdminWorkoutQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    // Count total workouts with filters
    let count_query = r#"
        SELECT COUNT(DISTINCT wd.id)
        FROM workout_data wd
        JOIN users u ON u.id = wd.user_id
        WHERE ($1::uuid IS NULL OR wd.user_id = $1)
        AND ($2::text IS NULL OR LOWER(u.username) LIKE LOWER(CONCAT('%', $2, '%')))
    "#;

    let total = sqlx::query_scalar::<_, i64>(count_query)
        .bind(query.user_id)
        .bind(&query.username)
        .fetch_one(pool.get_ref())
        .await
        .map_err(|e| {
            tracing::error!("Failed to count workouts: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to count workouts")
        })?;

    // Fetch workouts with user information
    let fetch_query = r#"
        SELECT 
            wd.id,
            wd.user_id,
            u.username,
            wd.device_id,
            COALESCE(jsonb_array_length(wd.heart_rate_data), 0) as heart_rate_count,
            wd.calories_burned,
            wd.workout_uuid,
            wd.workout_start,
            wd.workout_end,
            wd.created_at
        FROM workout_data wd
        JOIN users u ON u.id = wd.user_id
        WHERE ($1::uuid IS NULL OR wd.user_id = $1)
        AND ($2::text IS NULL OR LOWER(u.username) LIKE LOWER(CONCAT('%', $2, '%')))
        ORDER BY wd.created_at DESC
        LIMIT $3 OFFSET $4
    "#;

    let workouts: Vec<AdminWorkoutData> = sqlx::query_as(fetch_query)
        .bind(query.user_id)
        .bind(&query.username)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch workouts: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to fetch workouts")
        })?;

    Ok(HttpResponse::Ok().json(AdminWorkoutResponse { workouts, total }))
}

pub async fn get_workout_detail(
    pool: web::Data<PgPool>,
    workout_id: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let workout_query = r#"
        SELECT 
            wd.id,
            wd.user_id,
            u.username,
            wd.device_id,
            wd.heart_rate_data,
            wd.calories_burned,
            wd.workout_uuid,
            wd.workout_start,
            wd.workout_end,
            wd.created_at
        FROM workout_data wd
        JOIN users u ON u.id = wd.user_id
        WHERE wd.id = $1
    "#;

    let row = sqlx::query(workout_query)
        .bind(workout_id.into_inner())
        .fetch_one(pool.get_ref())
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => {
                actix_web::error::ErrorNotFound("Workout not found")
            }
            _ => {
                tracing::error!("Failed to fetch workout detail: {}", e);
                actix_web::error::ErrorInternalServerError("Failed to fetch workout detail")
            }
        })?;

    let heart_rate_json: Option<serde_json::Value> = row.get("heart_rate_data");
    let heart_rate = heart_rate_json.and_then(|json| {
        // Try to deserialize the heart rate data
        // Handle both possible formats: {heart_rate: ...} and {hr: ...}
        if let serde_json::Value::Array(arr) = json {
            let mut heart_rate_vec = Vec::new();
            for item in arr {
                if let serde_json::Value::Object(obj) = item {
                    if let (Some(timestamp), Some(hr_value)) = (
                        obj.get("timestamp").and_then(|t| t.as_str()),
                        obj.get("heart_rate").or_else(|| obj.get("hr")).and_then(|h| h.as_i64())
                    ) {
                        if let Ok(ts) = DateTime::parse_from_rfc3339(timestamp) {
                            heart_rate_vec.push(HeartRateData {
                                timestamp: ts.with_timezone(&Utc),
                                heart_rate: hr_value as i32,
                            });
                        }
                    }
                }
            }
            if !heart_rate_vec.is_empty() {
                Some(heart_rate_vec)
            } else {
                None
            }
        } else {
            None
        }
    });

    let workout = AdminWorkoutDetail {
        id: row.get("id"),
        user_id: row.get("user_id"),
        username: row.get("username"),
        device_id: row.get("device_id"),
        heart_rate,
        calories_burned: row.get("calories_burned"),
        workout_uuid: row.get("workout_uuid"),
        workout_start: row.get("workout_start"),
        workout_end: row.get("workout_end"),
        created_at: row.get("created_at"),
    };

    Ok(HttpResponse::Ok().json(workout))
}

pub async fn delete_workout(
    pool: web::Data<PgPool>,
    workout_id: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let workout_id = workout_id.into_inner();

    // First check if workout exists
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM workout_data WHERE id = $1)"
    )
    .bind(workout_id)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!("Failed to check workout existence: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to check workout")
    })?;

    if !exists {
        return Err(actix_web::error::ErrorNotFound("Workout not found"));
    }

    // Delete the workout
    sqlx::query("DELETE FROM workout_data WHERE id = $1")
        .bind(workout_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete workout: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to delete workout")
        })?;

    tracing::info!("Admin deleted workout: {}", workout_id);

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Workout deleted successfully",
        "workout_id": workout_id
    })))
}

#[derive(Debug, Deserialize)]
pub struct BulkDeleteRequest {
    pub workout_ids: Vec<Uuid>,
}

pub async fn bulk_delete_workouts(
    pool: web::Data<PgPool>,
    body: web::Json<BulkDeleteRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if body.workout_ids.is_empty() {
        return Err(actix_web::error::ErrorBadRequest("No workout IDs provided"));
    }

    // Delete all workouts in the list
    let result = sqlx::query(
        "DELETE FROM workout_data WHERE id = ANY($1)"
    )
    .bind(&body.workout_ids)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!("Failed to bulk delete workouts: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to delete workouts")
    })?;

    let deleted_count = result.rows_affected();
    
    tracing::info!("Admin bulk deleted {} workouts", deleted_count);

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("{} workouts deleted successfully", deleted_count),
        "deleted_count": deleted_count
    })))
}