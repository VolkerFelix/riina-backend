use actix_web::{web, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use std::sync::Arc;

use crate::models::workout_data::HeartRateData;
use crate::services::live_game_service::LiveGameService;

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
    redis_client: Option<web::Data<Arc<redis::Client>>>,
) -> Result<HttpResponse, actix_web::Error> {
    let workout_id = workout_id.into_inner();

    // First check if workout exists and get associated live game and stat change info
    let workout_info = sqlx::query(
        r#"
        SELECT 
            wd.id,
            wd.user_id as workout_user_id,
            wd.stamina_gained as workout_stamina_gained,
            wd.strength_gained as workout_strength_gained,
            lse.live_game_id,
            lse.team_side,
            lse.score_points,
            lse.power_contribution,
            lse.user_id
        FROM workout_data wd
        LEFT JOIN live_score_events lse ON lse.workout_data_id = wd.id
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
    let stamina_gained: Option<i32> = workout_row.try_get("workout_stamina_gained").ok();
    let strength_gained: Option<i32> = workout_row.try_get("workout_strength_gained").ok();
    let live_game_id: Option<Uuid> = workout_row.try_get("live_game_id").ok();
    let team_side: Option<String> = workout_row.try_get("team_side").ok();
    let score_points: Option<i32> = workout_row.try_get("score_points").ok();
    let power_contribution: Option<i32> = workout_row.try_get("power_contribution").ok();
    let user_id: Option<Uuid> = workout_row.try_get("user_id").ok();

    // Reverse stat changes from user's avatar if they exist
    if let (Some(stamina_gained), Some(strength_gained)) = (stamina_gained, strength_gained) {
        if stamina_gained != 0 || strength_gained != 0 {
            tracing::info!("Reversing stat changes for user {}: -{} stamina, -{} strength", 
                         workout_user_id, stamina_gained, strength_gained);
            
            sqlx::query!(
                r#"
                UPDATE user_avatars 
                SET stamina = GREATEST(0, stamina - $1), 
                    strength = GREATEST(0, strength - $2)
                WHERE user_id = $3
                "#,
                stamina_gained,
                strength_gained,
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

    // Delete the workout (this will cascade delete the live_score_events)
    sqlx::query("DELETE FROM workout_data WHERE id = $1")
        .bind(workout_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete workout: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to delete workout")
        })?;

    // If this workout was part of a live game, recalculate the scores
    if let (Some(live_game_id), Some(team_side), Some(score_points), Some(power_contribution), Some(user_id)) = 
        (live_game_id, team_side, score_points, power_contribution, user_id) {
        
        tracing::info!("Recalculating live game {} scores after workout deletion", live_game_id);
        
        // Update the live game scores by subtracting the deleted workout's contribution
        let update_query = if team_side == "home" {
            r#"
            UPDATE live_games 
            SET 
                home_score = GREATEST(0, home_score - $1),
                home_power = GREATEST(0, home_power - $2),
                updated_at = NOW()
            WHERE id = $3
            RETURNING *
            "#
        } else {
            r#"
            UPDATE live_games 
            SET 
                away_score = GREATEST(0, away_score - $1),
                away_power = GREATEST(0, away_power - $2),
                updated_at = NOW()
            WHERE id = $3
            RETURNING *
            "#
        };

        let updated_game = sqlx::query(update_query)
            .bind(score_points)
            .bind(power_contribution)
            .bind(live_game_id)
            .fetch_optional(pool.get_ref())
            .await
            .map_err(|e| {
                tracing::error!("Failed to update live game scores: {}", e);
                actix_web::error::ErrorInternalServerError("Failed to update live game scores")
            })?;

        // Broadcast the updated scores if the game was updated
        if updated_game.is_some() {
            let live_game_service = LiveGameService::new(pool.get_ref().clone(), redis_client.unwrap().get_ref().clone());
            
            // Get the updated live game and broadcast the change
            if let Ok(Some(live_game)) = live_game_service.get_live_game_by_id(live_game_id).await {
                let _ = live_game_service.broadcast_live_score_update(&live_game).await;
            }
        }

        // Update player contribution
        sqlx::query(
            r#"
            UPDATE live_player_contributions 
            SET 
                current_power = GREATEST(0, current_power - $1),
                total_score_contribution = GREATEST(0, total_score_contribution - $2),
                contribution_count = GREATEST(0, contribution_count - 1),
                updated_at = NOW()
            WHERE live_game_id = $3 AND user_id = $4
            "#
        )
        .bind(power_contribution)
        .bind(score_points)
        .bind(live_game_id)
        .bind(user_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            tracing::error!("Failed to update player contributions: {}", e);
            e
        })
        .ok();
    }

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
    redis_client: web::Data<Arc<redis::Client>>,
) -> Result<HttpResponse, actix_web::Error> {
    if body.workout_ids.is_empty() {
        return Err(actix_web::error::ErrorBadRequest("No workout IDs provided"));
    }

    // First, get info about all workouts that will be deleted and their live game associations
    let workout_infos = sqlx::query(
        r#"
        SELECT 
            wd.id as workout_id,
            wd.user_id as workout_user_id,
            wd.stamina_gained as workout_stamina_gained,
            wd.strength_gained as workout_strength_gained,
            lse.live_game_id,
            lse.team_side,
            lse.score_points,
            lse.power_contribution,
            lse.user_id
        FROM workout_data wd
        LEFT JOIN live_score_events lse ON lse.workout_data_id = wd.id
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

    // Group by live game to aggregate score changes
    use std::collections::HashMap;
    #[derive(Default)]
    struct GameScoreAdjustment {
        home_score_decrease: i32,
        home_power_decrease: i32,
        away_score_decrease: i32,
        away_power_decrease: i32,
        user_contributions: HashMap<Uuid, (i32, i32)>, // user_id -> (score, power)
    }
    
    let mut game_adjustments: HashMap<Uuid, GameScoreAdjustment> = HashMap::new();
    
    for row in &workout_infos {
        if let (Ok(Some(live_game_id)), Ok(Some(team_side)), Ok(Some(score)), Ok(Some(power)), Ok(Some(user_id))) = (
            row.try_get::<Option<Uuid>, _>("live_game_id"),
            row.try_get::<Option<String>, _>("team_side"),
            row.try_get::<Option<i32>, _>("score_points"),
            row.try_get::<Option<i32>, _>("power_contribution"),
            row.try_get::<Option<Uuid>, _>("user_id"),
        ) {
            let adjustment = game_adjustments.entry(live_game_id).or_default();
            
            if team_side == "home" {
                adjustment.home_score_decrease += score;
                adjustment.home_power_decrease += power;
            } else {
                adjustment.away_score_decrease += score;
                adjustment.away_power_decrease += power;
            }
            
            let user_contrib = adjustment.user_contributions.entry(user_id).or_default();
            user_contrib.0 += score;
            user_contrib.1 += power;
        }
    }

    // Group and reverse stat changes by user
    let mut user_stat_changes: HashMap<Uuid, (i32, i32)> = HashMap::new(); // user_id -> (stamina, strength)
    
    for row in &workout_infos {
        let workout_user_id: Uuid = row.get("workout_user_id");
        if let (Ok(Some(stamina_gained)), Ok(Some(strength_gained))) = (
            row.try_get::<Option<i32>, _>("workout_stamina_gained"),
            row.try_get::<Option<i32>, _>("workout_strength_gained"),
        ) {
            if stamina_gained != 0 || strength_gained != 0 {
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
            stamina_to_subtract,
            strength_to_subtract,
            user_id
        )
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            tracing::error!("Failed to reverse bulk user stat changes for {}: {}", user_id, e);
            actix_web::error::ErrorInternalServerError("Failed to reverse stat changes")
        })?;
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
    
    // Now update all affected live games
    let live_game_service = LiveGameService::new(pool.get_ref().clone(), redis_client.get_ref().clone());
    
    for (live_game_id, adjustment) in game_adjustments {
        // Update live game scores
        let update_result = sqlx::query(
            r#"
            UPDATE live_games 
            SET 
                home_score = GREATEST(0, home_score - $1),
                home_power = GREATEST(0, home_power - $2),
                away_score = GREATEST(0, away_score - $3),
                away_power = GREATEST(0, away_power - $4),
                updated_at = NOW()
            WHERE id = $5
            RETURNING id
            "#
        )
        .bind(adjustment.home_score_decrease)
        .bind(adjustment.home_power_decrease)
        .bind(adjustment.away_score_decrease)
        .bind(adjustment.away_power_decrease)
        .bind(live_game_id)
        .fetch_optional(pool.get_ref())
        .await;

        if update_result.is_ok() && update_result.unwrap().is_some() {
            // Update player contributions
            for (user_id, (score_decrease, power_decrease)) in adjustment.user_contributions {
                sqlx::query(
                    r#"
                    UPDATE live_player_contributions 
                    SET 
                        current_power = GREATEST(0, current_power - $1),
                        total_score_contribution = GREATEST(0, total_score_contribution - $2),
                        contribution_count = GREATEST(0, contribution_count - 1),
                        updated_at = NOW()
                    WHERE live_game_id = $3 AND user_id = $4
                    "#
                )
                .bind(power_decrease)
                .bind(score_decrease)
                .bind(live_game_id)
                .bind(user_id)
                .execute(pool.get_ref())
                .await
                .ok();
            }

            // Broadcast the updated scores
            if let Ok(Some(live_game)) = live_game_service.get_live_game_by_id(live_game_id).await {
                let _ = live_game_service.broadcast_live_score_update(&live_game).await;
            }
        }
    }
    
    tracing::info!("Admin bulk deleted {} workouts", deleted_count);

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("{} workouts deleted successfully", deleted_count),
        "deleted_count": deleted_count
    })))
}