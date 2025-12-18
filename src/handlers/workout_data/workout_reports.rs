use actix_web::{web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;

use crate::models::workout_data::{SubmitWorkoutReportRequest, UpdateWorkoutReportRequest, WorkoutReport};
use crate::middleware::auth::Claims;
use crate::handlers::notification_handler::send_notification_to_user;
use crate::services::social_events::send_websocket_notification_to_user;

/// Submit a report for a suspicious workout
/// POST /workouts/{workout_id}/report
pub async fn submit_workout_report(
    pool: web::Data<PgPool>,
    workout_id: web::Path<Uuid>,
    claims: web::ReqData<Claims>,
    request: web::Json<SubmitWorkoutReportRequest>,
) -> Result<HttpResponse> {
    let workout_id = workout_id.into_inner();
    let reporter_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid user ID in token"
            })));
        }
    };

    // Validate request
    if let Err(e) = request.validate() {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": e
        })));
    }

    // Get the workout and verify it exists
    let workout = sqlx::query!(
        r#"
        SELECT id, user_id
        FROM workout_data
        WHERE id = $1
        "#,
        workout_id
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error checking workout: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to verify workout")
    })?;

    let workout = match workout {
        Some(w) => w,
        None => {
            return Ok(HttpResponse::NotFound().json(serde_json::json!({
                "error": "Workout not found"
            })));
        }
    };

    let workout_owner_id = workout.user_id;

    // Check if this workout has already been reported
    let existing_report = sqlx::query!(
        r#"
        SELECT id, reported_by_user_id
        FROM workout_reports
        WHERE workout_data_id = $1
        "#,
        workout_id
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error checking existing reports: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to check existing reports")
    })?;

    if let Some(existing) = existing_report {
        // If the same user is reporting again, allow them to update their report
        if existing.reported_by_user_id == reporter_id {
            let report = sqlx::query_as!(
                WorkoutReport,
                r#"
                UPDATE workout_reports
                SET reason = $1
                WHERE id = $2
                RETURNING
                    id,
                    workout_data_id,
                    reported_by_user_id,
                    workout_owner_id,
                    reason,
                    status,
                    admin_notes,
                    reviewed_by_user_id,
                    reviewed_at,
                    created_at
                "#,
                request.reason,
                existing.id
            )
            .fetch_one(pool.as_ref())
            .await
            .map_err(|e| {
                tracing::error!("Database error updating report: {}", e);
                actix_web::error::ErrorInternalServerError("Failed to update report")
            })?;

            return Ok(HttpResponse::Ok().json(report));
        } else {
            // Different user trying to report - return conflict
            return Ok(HttpResponse::Conflict().json(serde_json::json!({
                "error": "This workout has already been reported and is under review",
                "reported": true
            })));
        }
    }

    // Insert new report
    let report = sqlx::query_as!(
        WorkoutReport,
        r#"
        INSERT INTO workout_reports (workout_data_id, reported_by_user_id, workout_owner_id, reason)
        VALUES ($1, $2, $3, $4)
        RETURNING
            id,
            workout_data_id,
            reported_by_user_id,
            workout_owner_id,
            reason,
            status,
            admin_notes,
            reviewed_by_user_id,
            reviewed_at,
            created_at
        "#,
        workout_id,
        reporter_id,
        workout_owner_id,
        request.reason
    )
    .fetch_one(pool.as_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error submitting report: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to submit report")
    })?;

    Ok(HttpResponse::Ok().json(report))
}

/// Get the current user's report for a specific workout
/// GET /workouts/{workout_id}/my-report
pub async fn get_my_report_for_workout(
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

    let report = sqlx::query_as!(
        WorkoutReport,
        r#"
        SELECT
            id,
            workout_data_id,
            reported_by_user_id,
            workout_owner_id,
            reason,
            status,
            admin_notes,
            reviewed_by_user_id,
            reviewed_at,
            created_at
        FROM workout_reports
        WHERE workout_data_id = $1 AND reported_by_user_id = $2
        "#,
        workout_id,
        user_id
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error fetching report: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch report")
    })?;

    match report {
        Some(r) => Ok(HttpResponse::Ok().json(r)),
        None => Ok(HttpResponse::Ok().json(serde_json::json!({
            "report": null
        }))),
    }
}

/// Get all reports submitted by the current user
/// GET /workouts/reports/my-reports
pub async fn get_my_reports(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid user ID in token"
            })));
        }
    };

    let reports = sqlx::query_as!(
        WorkoutReport,
        r#"
        SELECT
            id,
            workout_data_id,
            reported_by_user_id,
            workout_owner_id,
            reason,
            status,
            admin_notes,
            reviewed_by_user_id,
            reviewed_at,
            created_at
        FROM workout_reports
        WHERE reported_by_user_id = $1
        ORDER BY created_at DESC
        "#,
        user_id
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error fetching user reports: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch reports")
    })?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "reports": reports,
        "count": reports.len()
    })))
}

/// Delete a report (only the reporter can delete their own report)
/// DELETE /workouts/reports/{report_id}
pub async fn delete_workout_report(
    pool: web::Data<PgPool>,
    report_id: web::Path<Uuid>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let report_id = report_id.into_inner();
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid user ID in token"
            })));
        }
    };

    // Delete the report if it belongs to the user
    let result = sqlx::query!(
        r#"
        DELETE FROM workout_reports
        WHERE id = $1 AND reported_by_user_id = $2
        "#,
        report_id,
        user_id
    )
    .execute(pool.as_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error deleting report: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to delete report")
    })?;

    if result.rows_affected() == 0 {
        return Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Report not found or you don't have permission to delete it"
        })));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Report deleted successfully"
    })))
}

/// Update report status (admin only)
/// PATCH /admin/workout-reports/{report_id}
pub async fn update_report_status(
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<redis::Client>>,
    report_id: web::Path<Uuid>,
    claims: web::ReqData<Claims>,
    request: web::Json<UpdateWorkoutReportRequest>,
) -> Result<HttpResponse> {
    let report_id = report_id.into_inner();
    let admin_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid user ID in token"
            })));
        }
    };

    // Validate request
    if let Err(e) = request.validate() {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": e
        })));
    }

    // Update the report
    let report = sqlx::query_as!(
        WorkoutReport,
        r#"
        UPDATE workout_reports
        SET
            status = $1,
            admin_notes = $2,
            reviewed_by_user_id = $3,
            reviewed_at = NOW()
        WHERE id = $4
        RETURNING
            id,
            workout_data_id,
            reported_by_user_id,
            workout_owner_id,
            reason,
            status,
            admin_notes,
            reviewed_by_user_id,
            reviewed_at,
            created_at
        "#,
        request.status,
        request.admin_notes,
        admin_id,
        report_id
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error updating report: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to update report")
    })?;

    match report {
        Some(r) => {
            // Send notifications to the reporter if status is confirmed or dismissed
            if request.status == "confirmed" || request.status == "dismissed" {
                let (title, body) = match request.status.as_str() {
                    "confirmed" => (
                        "Reported Issue Confirmed".to_string(),
                        "Your reported workout has been reviewed and confirmed by an admin. Thank you for helping maintain fair play!".to_string()
                    ),
                    "dismissed" => (
                        "Reported Issue Reviewed".to_string(),
                        "Your reported workout has been reviewed. After investigation, the workout appears to be legitimate.".to_string()
                    ),
                    _ => ("Reported Issue Updated".to_string(), "Your reported workout status has been updated.".to_string()),
                };

                // Send notifications asynchronously - don't fail the request if notification fails
                let pool_clone = pool.get_ref().clone();
                let redis_clone = redis_client.clone();
                let reporter_id = r.reported_by_user_id;
                let report_id = r.id;
                let workout_id = r.workout_data_id;
                let status = request.status.clone();
                let admin_username = claims.username.clone();

                tokio::spawn(async move {
                    // Create notification in database first
                    let notification_message = format!("{} - {}", title, body);
                    match sqlx::query!(
                        r#"
                        INSERT INTO notifications (recipient_id, actor_id, notification_type, entity_type, entity_id, message)
                        VALUES ($1, $2, 'workout_report_reviewed', 'workout_report', $3, $4)
                        RETURNING id
                        "#,
                        reporter_id,
                        admin_id,
                        report_id,
                        notification_message
                    )
                    .fetch_one(&pool_clone)
                    .await {
                        Ok(notif_row) => {
                            tracing::info!("Created notification {} for workout report review", notif_row.id);

                            // Send WebSocket notification
                            let ws_message = format!("{} - {}", title, body);
                            if let Err(e) = send_websocket_notification_to_user(
                                &redis_clone,
                                reporter_id,
                                notif_row.id,
                                admin_username,
                                "workout_report".to_string(),
                                ws_message,
                            ).await {
                                tracing::error!("Failed to send WebSocket notification to user {}: {}", reporter_id, e);
                            } else {
                                tracing::info!("Successfully sent WebSocket notification to user {}", reporter_id);
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to create notification in database for user {}: {}", reporter_id, e);
                        }
                    }

                    // Send push notification
                    let notification_data = serde_json::json!({
                        "type": "workout_report_reviewed",
                        "report_id": report_id,
                        "workout_id": workout_id,
                        "status": status,
                    });

                    if let Err(e) = send_notification_to_user(
                        &pool_clone,
                        reporter_id,
                        title,
                        body,
                        Some(notification_data),
                        Some("workout_report".to_string()),
                    ).await {
                        tracing::error!("Failed to send push notification to user {}: {}", reporter_id, e);
                    } else {
                        tracing::info!("Successfully sent push notification to user {}", reporter_id);
                    }
                });
            }

            Ok(HttpResponse::Ok().json(r))
        },
        None => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Report not found"
        }))),
    }
}

/// Get all pending reports (admin only)
/// GET /admin/workout-reports/pending
pub async fn get_pending_reports(
    pool: web::Data<PgPool>,
    _claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let reports = sqlx::query_as!(
        WorkoutReport,
        r#"
        SELECT
            id,
            workout_data_id,
            reported_by_user_id,
            workout_owner_id,
            reason,
            status,
            admin_notes,
            reviewed_by_user_id,
            reviewed_at,
            created_at
        FROM workout_reports
        WHERE status = 'pending'
        ORDER BY created_at ASC
        "#
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error fetching pending reports: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch pending reports")
    })?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "reports": reports,
        "count": reports.len()
    })))
}

/// Get all reports (admin only)
/// GET /admin/workout-reports
pub async fn get_all_reports(
    pool: web::Data<PgPool>,
    _claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let reports = sqlx::query_as!(
        WorkoutReport,
        r#"
        SELECT
            id,
            workout_data_id,
            reported_by_user_id,
            workout_owner_id,
            reason,
            status,
            admin_notes,
            reviewed_by_user_id,
            reviewed_at,
            created_at
        FROM workout_reports
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error fetching all reports: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch reports")
    })?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "reports": reports,
        "count": reports.len()
    })))
}
