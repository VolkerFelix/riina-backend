use actix_web::{web, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::models::workout_data::HeartRateData;
use crate::models::common::ApiResponse;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AdminWorkoutData {
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
pub struct AdminWorkoutDetail {
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
    pub stamina_gained: Option<f32>,
    pub strength_gained: Option<f32>,
    pub zone_breakdown: Option<serde_json::Value>,
    pub ml_prediction: Option<String>,
    pub ml_confidence: Option<f32>,
    pub ml_classified_at: Option<DateTime<Utc>>,
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

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: "Workouts retrieved successfully".to_string(),
        data: Some(AdminWorkoutResponse { workouts, total }),
        error: None,
    }))
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
            wd.created_at,
            wd.stamina_gained,
            wd.strength_gained,
            wd.heart_rate_zones,
            wd.ml_prediction,
            wd.ml_confidence,
            wd.ml_classified_at
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
        stamina_gained: row.get("stamina_gained"),
        strength_gained: row.get("strength_gained"),
        zone_breakdown: row.get("heart_rate_zones"),
        ml_prediction: row.get("ml_prediction"),
        ml_confidence: row.get("ml_confidence"),
        ml_classified_at: row.get("ml_classified_at"),
    };

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: "Workout details retrieved successfully".to_string(),
        data: Some(workout),
        error: None,
    }))
}

pub async fn delete_workout(
    pool: web::Data<PgPool>,
    workout_id: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let workout_id = workout_id.into_inner();

    // First check if workout exists and get stat change info
    let workout_info = sqlx::query(
        r#"
        SELECT 
            wd.id,
            wd.user_id as workout_user_id,
            wd.stamina_gained as workout_stamina_gained,
            wd.strength_gained as workout_strength_gained
        FROM workout_data wd
        WHERE wd.id = $1
        "#
    )
    .bind(workout_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!("Failed to check workout existence: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to check workout")
    })?;

    if workout_info.is_none() {
        return Err(actix_web::error::ErrorNotFound("Workout not found"));
    }

    let workout_row = workout_info.unwrap();
    let workout_user_id: Uuid = workout_row.get("workout_user_id");
    let stamina_gained: Option<f32> = workout_row.try_get("workout_stamina_gained").ok();
    let strength_gained: Option<f32> = workout_row.try_get("workout_strength_gained").ok();

    // Reverse stat changes from user's avatar if they exist
    if let (Some(stamina_gained), Some(strength_gained)) = (stamina_gained, strength_gained) {
        if stamina_gained != 0.0 || strength_gained != 0.0 {
            tracing::info!("Reversing stat changes for user {}: -{} stamina, -{} strength", 
                         workout_user_id, stamina_gained, strength_gained);
            
            sqlx::query!(
                r#"
                UPDATE user_avatars 
                SET stamina = GREATEST(0, stamina - $1), 
                    strength = GREATEST(0, strength - $2)
                WHERE user_id = $3
                "#,
                stamina_gained as f32,
                strength_gained as f32,
                workout_user_id
            )
            .execute(pool.get_ref())
            .await
            .map_err(|e| {
                tracing::error!("Failed to reverse user stat changes: {}", e);
                actix_web::error::ErrorInternalServerError("Failed to reverse stat changes")
            })?;
        }
    }

    // Before deleting the workout, recalculate live game scores
    if let Err(e) = recalculate_live_game_scores_after_workout_deletion(workout_id, pool.get_ref()).await {
        tracing::error!("Failed to recalculate live game scores after workout deletion: {}", e);
        // Don't fail the deletion for this, just log the error
    }

    // Delete the workout (this will cascade delete any related entries)
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

    // First, get info about all workouts that will be deleted
    let workout_infos = sqlx::query(
        r#"
        SELECT 
            wd.id as workout_id,
            wd.user_id as workout_user_id,
            wd.stamina_gained as workout_stamina_gained,
            wd.strength_gained as workout_strength_gained
        FROM workout_data wd
        WHERE wd.id = ANY($1)
        "#
    )
    .bind(&body.workout_ids)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch workout info: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch workout info")
    })?;

    // Group and reverse stat changes by user
    use std::collections::HashMap;
    let mut user_stat_changes: HashMap<Uuid, (f32, f32)> = HashMap::new(); // user_id -> (stamina, strength)
    
    for row in &workout_infos {
        let workout_user_id: Uuid = row.get("workout_user_id");
        if let (Ok(Some(stamina_gained)), Ok(Some(strength_gained))) = (
            row.try_get::<Option<f32>, _>("workout_stamina_gained"),
            row.try_get::<Option<f32>, _>("workout_strength_gained"),
        ) {
            if stamina_gained != 0.0 || strength_gained != 0.0 {
                let user_stats = user_stat_changes.entry(workout_user_id).or_default();
                user_stats.0 += stamina_gained;
                user_stats.1 += strength_gained;
            }
        }
    }
    
    // Reverse all user stat changes
    for (user_id, (stamina_to_subtract, strength_to_subtract)) in user_stat_changes {
        tracing::info!("Reversing bulk stat changes for user {}: -{} stamina, -{} strength", 
                     user_id, stamina_to_subtract, strength_to_subtract);
        
        sqlx::query!(
            r#"
            UPDATE user_avatars 
            SET stamina = GREATEST(0, stamina - $1), 
                strength = GREATEST(0, strength - $2)
            WHERE user_id = $3
            "#,
            stamina_to_subtract as f32,
            strength_to_subtract as f32,
            user_id
        )
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            tracing::error!("Failed to reverse bulk user stat changes for {}: {}", user_id, e);
            actix_web::error::ErrorInternalServerError("Failed to reverse stat changes")
        })?;
    }

    // Recalculate live game scores for each workout before deletion
    for workout_id in &body.workout_ids {
        if let Err(e) = recalculate_live_game_scores_after_workout_deletion(*workout_id, pool.get_ref()).await {
            tracing::error!("Failed to recalculate live game scores for workout {}: {}", workout_id, e);
            // Don't fail the bulk deletion for this, just log the error
        }
    }

    // Delete all workouts in the list (this will cascade delete live_score_events)
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

/// Recalculate live game scores after a workout deletion
async fn recalculate_live_game_scores_after_workout_deletion(
    workout_id: Uuid,
    pool: &sqlx::PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🔄 Recalculating live game scores after workout deletion: {}", workout_id);

    // Find all games that had score events from this workout
    let affected_games = sqlx::query!(
        r#"
        SELECT DISTINCT lse.game_id
        FROM live_score_events lse 
        WHERE lse.workout_data_id = $1
        AND lse.game_id IS NOT NULL
        "#,
        workout_id
    )
    .fetch_all(pool)
    .await?;

    for game_record in affected_games {
        let game_id = game_record.game_id.expect("game_id should not be null due to WHERE clause");
        tracing::info!("🔄 Recalculating scores for game: {}", game_id);

        // Delete the old score events for this workout
        sqlx::query!(
            "DELETE FROM live_score_events WHERE workout_data_id = $1",
            workout_id
        )
        .execute(pool)
        .await?;

        // Recalculate total scores for both teams from remaining score events
        let score_totals = sqlx::query!(
            r#"
            SELECT 
                COALESCE(SUM(CASE WHEN team_side = 'home' THEN score_points ELSE 0 END), 0) as home_total,
                COALESCE(SUM(CASE WHEN team_side = 'away' THEN score_points ELSE 0 END), 0) as away_total
            FROM live_score_events 
            WHERE game_id = $1
            "#,
            game_id
        )
        .fetch_one(pool)
        .await?;

        // Update the game with recalculated scores
        sqlx::query!(
            r#"
            UPDATE games 
            SET home_score = $1, away_score = $2
            WHERE id = $3
            "#,
            score_totals.home_total.unwrap_or(0.0) as i32,
            score_totals.away_total.unwrap_or(0.0) as i32,
            game_id
        )
        .execute(pool)
        .await?;

        tracing::info!("✅ Updated game {} scores: home={}, away={}", 
            game_id, score_totals.home_total.unwrap_or(0.0), score_totals.away_total.unwrap_or(0.0));
    }

    Ok(())
}