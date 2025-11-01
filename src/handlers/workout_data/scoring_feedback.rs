use actix_web::{web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::workout_data::{SubmitScoringFeedbackRequest, ScoringFeedbackType, WorkoutScoringFeedback};
use crate::middleware::auth::Claims;

/// Submit scoring feedback for a workout
/// POST /workouts/{workout_id}/scoring-feedback
pub async fn submit_scoring_feedback(
    pool: web::Data<PgPool>,
    workout_id: web::Path<Uuid>,
    claims: web::ReqData<Claims>,
    request: web::Json<SubmitScoringFeedbackRequest>,
) -> Result<HttpResponse> {
    let workout_id = workout_id.into_inner();
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid user ID in token"
            })));
        }
    };
    let feedback_type = request.feedback_type;

    // Verify the workout exists and belongs to this user
    let workout_exists = sqlx::query!(
        r#"
        SELECT id FROM workout_data
        WHERE id = $1 AND user_id = $2
        "#,
        workout_id,
        user_id
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error checking workout: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to verify workout")
    })?;

    if workout_exists.is_none() {
        return Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Workout not found or you don't have permission to rate it"
        })));
    }

    // Insert or update feedback (upsert)
    let feedback_type_str = match feedback_type {
        ScoringFeedbackType::TooHigh => "too_high",
        ScoringFeedbackType::TooLow => "too_low",
        ScoringFeedbackType::Accurate => "accurate",
    };

    let feedback = sqlx::query_as!(
        WorkoutScoringFeedback,
        r#"
        INSERT INTO workout_scoring_feedback (workout_data_id, user_id, feedback_type)
        VALUES ($1, $2, $3)
        ON CONFLICT (workout_data_id, user_id)
        DO UPDATE SET
            feedback_type = $3,
            created_at = NOW()
        RETURNING
            id,
            workout_data_id,
            user_id,
            feedback_type as "feedback_type: ScoringFeedbackType",
            created_at
        "#,
        workout_id,
        user_id,
        feedback_type_str
    )
    .fetch_one(pool.as_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error submitting feedback: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to submit feedback")
    })?;

    Ok(HttpResponse::Ok().json(feedback))
}

/// Get scoring feedback for a workout
/// GET /workouts/{workout_id}/scoring-feedback
pub async fn get_scoring_feedback(
    pool: web::Data<PgPool>,
    workout_id: web::Path<Uuid>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let workout_id = workout_id.into_inner();
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid user ID in token"
            })));
        }
    };

    // Verify the workout exists and belongs to this user
    let workout_exists = sqlx::query!(
        r#"
        SELECT id FROM workout_data
        WHERE id = $1 AND user_id = $2
        "#,
        workout_id,
        user_id
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error checking workout: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to verify workout")
    })?;

    if workout_exists.is_none() {
        return Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Workout not found or you don't have permission to view it"
        })));
    }

    // Get feedback if it exists
    let feedback = sqlx::query_as!(
        WorkoutScoringFeedback,
        r#"
        SELECT
            id,
            workout_data_id,
            user_id,
            feedback_type as "feedback_type: ScoringFeedbackType",
            created_at
        FROM workout_scoring_feedback
        WHERE workout_data_id = $1 AND user_id = $2
        "#,
        workout_id,
        user_id
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error fetching feedback: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch feedback")
    })?;

    match feedback {
        Some(f) => Ok(HttpResponse::Ok().json(f)),
        None => Ok(HttpResponse::Ok().json(serde_json::json!({
            "feedback": null
        }))),
    }
}
