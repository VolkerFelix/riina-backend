use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::middleware::auth::Claims;
use crate::models::common::ApiResponse;
use crate::db::workout_data::check_workout_exists_by_time;

#[derive(Debug, Deserialize)]
pub struct WorkoutTimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub id: String,  // Keep original ID for frontend reference
}

#[derive(Debug, Deserialize)]
pub struct CheckSyncStatusRequest {
    pub workouts: Vec<WorkoutTimeRange>,
}

#[derive(Debug, Serialize)]
pub struct SyncStatusResponse {
    pub unsynced_workouts: Vec<String>,  // IDs of workouts to be synced
}

#[tracing::instrument(
    name = "Check workout sync status",
    skip(pool, claims, request),
    fields(
        username = %claims.username,
        workout_count = %request.workouts.len()
    )
)]
pub async fn check_workout_sync_status(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    request: web::Json<CheckSyncStatusRequest>,
) -> HttpResponse {
    tracing::info!("🎮 Checking workout sync status for user: {} ({} workouts)", 
        claims.username, request.workouts.len());

    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => {
            tracing::info!("User ID parsed successfully: {}", id);
            id
        },
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };

    let mut synced_workouts = Vec::new();
    let mut unsynced_workouts = Vec::new();

    // Check each workout using the time-based duplicate detection function
    for workout in &request.workouts {
        match check_workout_exists_by_time(pool.get_ref(), user_id, &workout.start, &workout.end).await {
            Ok(exists) => {
                if exists {
                    tracing::debug!("Workout {} already synced (time match)", workout.id);
                    synced_workouts.push(workout.id.clone());
                } else {
                    tracing::debug!("Workout {} not synced", workout.id);
                    unsynced_workouts.push(workout.id.clone());
                }
            },
            Err(e) => {
                tracing::error!("Error checking workout {}: {}", workout.id, e);
                // Treat as unsynced on error to allow retry
                unsynced_workouts.push(workout.id.clone());
            }
        }
    }

    let response = SyncStatusResponse {
        unsynced_workouts,
    };

    tracing::info!("✅ Sync status check completed: {} synced, {} unsynced", 
        synced_workouts.len(), response.unsynced_workouts.len());
    
    HttpResponse::Ok().json(ApiResponse::success(
        "Sync status retrieved successfully",
        response,
    ))
}