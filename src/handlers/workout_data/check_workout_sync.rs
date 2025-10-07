use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};
use std::cmp::Ordering;

use crate::middleware::auth::Claims;
use crate::models::common::ApiResponse;
use crate::db::workout_data::check_workout_exists_by_time;
use crate::utils::workout_approval::WorkoutApprovalToken;
use crate::config::jwt::JwtSettings;

/// Time tolerance in seconds for workout duplicate detection
const WORKOUT_TIME_TOLERANCE: Duration = Duration::seconds(1);

#[derive(Debug, Deserialize, Clone)]
pub struct WorkoutSyncRequest {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub calories: Option<i32>,
    pub id: String,  // Keep original ID for frontend reference
}

#[derive(Debug, Deserialize)]
pub struct CheckSyncStatusRequest {
    pub workouts: Vec<WorkoutSyncRequest>,
}

#[derive(Debug, Serialize)]
pub struct WorkoutApproval {
    pub workout_id: String,
    pub approval_token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SyncStatusResponse {
    pub unsynced_workouts: Vec<String>,
    pub approved_workouts: Vec<WorkoutApproval>,  // New field with approval tokens
}

#[tracing::instrument(
    name = "Approve workout sync",
    skip(pool, claims, request),
    fields(
        username = %claims.username,
        workout_count = %request.workouts.len()
    )
)]
pub async fn check_workout_sync(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    request: web::Json<CheckSyncStatusRequest>,
    jwt_settings: web::Data<JwtSettings>,
) -> HttpResponse {
    tracing::info!("🎮 Checking workout sync for user: {} ({} workouts)", 
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
    let mut approved_workouts = Vec::new();

    let unique_workouts = remove_duplicates(request.workouts.clone(), WORKOUT_TIME_TOLERANCE);

    // Check each workout using the time-based duplicate detection function
    for workout in &unique_workouts {
        match check_workout_exists_by_time(pool.get_ref(), user_id, &workout.start, &workout.end, WORKOUT_TIME_TOLERANCE).await {
            Ok(exists) => {
                if exists {
                    tracing::debug!("Workout {} already synced (time match)", workout.id);
                    synced_workouts.push(workout.id.clone());
                } else {
                    tracing::debug!("Workout {} not synced", workout.id);
                    unsynced_workouts.push(workout.id.clone());
                    
                    // Generate approval token for this workout
                    let token_data = WorkoutApprovalToken::new(
                        user_id,
                        workout.id.clone(),
                        workout.start,
                        workout.end,
                        5, // 5 minutes validity
                    );
                    
                    match token_data.generate_token(&jwt_settings.secret) {
                        Ok(token) => {
                            approved_workouts.push(WorkoutApproval {
                                workout_id: workout.id.clone(),
                                approval_token: token,
                                expires_at: token_data.expires_at,
                            });
                        },
                        Err(e) => {
                            tracing::error!("Failed to generate approval token for workout {}: {}", workout.id, e);
                            // Still allow sync but without token validation
                        }
                    }
                }
            },
            Err(e) => {
                tracing::error!("Error checking workout {}: {}", workout.id, e);
                // Treat as unsynced on error to allow retry
                unsynced_workouts.push(workout.id.clone());
                
                // Generate approval token even on error to allow upload
                let token_data = WorkoutApprovalToken::new(
                    user_id,
                    workout.id.clone(),
                    workout.start,
                    workout.end,
                    5, // 5 minutes validity
                );
                
                match token_data.generate_token(&jwt_settings.secret) {
                    Ok(token) => {
                        approved_workouts.push(WorkoutApproval {
                            workout_id: workout.id.clone(),
                            approval_token: token,
                            expires_at: token_data.expires_at,
                        });
                    },
                    Err(e) => {
                        tracing::error!("Failed to generate approval token for workout {}: {}", workout.id, e);
                    }
                }
            }
        }
    }

    let response = SyncStatusResponse {
        unsynced_workouts,
        approved_workouts,
    };

    tracing::info!("✅ Sync status check completed: {} synced, {} unsynced, {} approved", 
        synced_workouts.len(), response.unsynced_workouts.len(), response.approved_workouts.len());
    
    HttpResponse::Ok().json(ApiResponse::success(
        "Sync status retrieved successfully",
        response,
    ))
}

fn remove_duplicates(mut workouts: Vec<WorkoutSyncRequest>, tolerance: Duration) -> Vec<WorkoutSyncRequest> {
    // Sort by start times, then by end times
    workouts.sort_unstable_by(|a, b| {
        match a.start.cmp(&b.start) {
            Ordering::Equal => a.end.cmp(&b.end),
            ordering => ordering, // Short for Ordering::Less => Ordering::Less, Ordering::Greater => Ordering::Greater
        }
    });

    let mut unique_workouts: Vec<WorkoutSyncRequest> = Vec::with_capacity(workouts.len());

    'outer: for workout in workouts {
        // Scan recent uniques in reverse; break early once overlaps can't happen
        for u in unique_workouts.iter_mut().rev() {
            if u.end + tolerance < workout.start {
                // Since unique is sorted by start and we scan backward,
                // earlier items will end even earlier -> no more overlaps possible.
                break;
            }
            if time_intervals_overlap((&u.start, &u.end), (&workout.start, &workout.end), tolerance) {
                if workout.calories.unwrap_or(0) > u.calories.unwrap_or(0) {
                    *u = workout.clone();
                }
                continue 'outer;
            }
        }
        unique_workouts.push(workout);
    }
    unique_workouts
}

// Check if two time intervals overlap
fn time_intervals_overlap(
    interval1: (&DateTime<Utc>, &DateTime<Utc>),
    interval2: (&DateTime<Utc>, &DateTime<Utc>),
    tolerance: Duration,
) -> bool {
    interval1.0 <= &(*interval2.1 + tolerance) && interval1.1 >= &(*interval2.0 - tolerance)
}