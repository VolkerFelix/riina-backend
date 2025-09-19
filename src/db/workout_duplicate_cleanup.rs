use sqlx::{Pool, Postgres};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct OverlappingWorkout {
    pub id: Uuid,
    pub workout_start: DateTime<Utc>,
    pub workout_end: DateTime<Utc>,
    pub calories_burned: Option<i32>,
    pub device_id: String,
    pub created_at: DateTime<Utc>,
}

/// Find all overlapping workouts for a user within a time period
/// Overlapping means workouts where the time ranges intersect
#[tracing::instrument(
    name = "Find overlapping workouts",
    skip(pool),
    fields(
        user_id = %user_id
    )
)]
pub async fn find_overlapping_workouts(
    pool: &Pool<Postgres>,
    user_id: Uuid,
) -> Result<Vec<Vec<OverlappingWorkout>>, sqlx::Error> {
    tracing::info!("Finding overlapping workouts for user {}", user_id);

    // Get all workouts for the user, ordered by start time
    let all_workouts = sqlx::query_as!(
        OverlappingWorkout,
        r#"
        SELECT
            id,
            workout_start,
            workout_end,
            calories_burned,
            device_id,
            created_at
        FROM workout_data
        WHERE user_id = $1
            AND workout_start IS NOT NULL
            AND workout_end IS NOT NULL
        ORDER BY workout_start, created_at
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    // Group overlapping workouts
    let mut overlap_groups: Vec<Vec<OverlappingWorkout>> = Vec::new();

    for workout in all_workouts {
        let mut added_to_group = false;

        // Check if this workout overlaps with any existing group
        for group in &mut overlap_groups {
            if group.iter().any(|w| workouts_overlap(&workout, w)) {
                group.push(workout.clone());
                added_to_group = true;
                break;
            }
        }

        // If not added to any group, create a new group
        if !added_to_group {
            overlap_groups.push(vec![workout]);
        }
    }

    // Filter out groups with only one workout (no duplicates)
    let duplicate_groups: Vec<Vec<OverlappingWorkout>> = overlap_groups
        .into_iter()
        .filter(|group| group.len() > 1)
        .collect();

    tracing::info!("Found {} groups of overlapping workouts", duplicate_groups.len());

    Ok(duplicate_groups)
}

/// Check if two workouts overlap in time
fn workouts_overlap(w1: &OverlappingWorkout, w2: &OverlappingWorkout) -> bool {
    // Two intervals [a1, a2] and [b1, b2] overlap if:
    // a1 <= b2 AND b1 <= a2
    w1.workout_start <= w2.workout_end && w2.workout_start <= w1.workout_end
}

/// Remove duplicate workouts keeping the one with highest calories
/// Returns the IDs of removed workouts
#[tracing::instrument(
    name = "Remove duplicate workouts",
    skip(pool, duplicates),
    fields(
        duplicate_count = duplicates.len()
    )
)]
pub async fn remove_duplicate_workouts(
    pool: &Pool<Postgres>,
    duplicates: Vec<OverlappingWorkout>,
) -> Result<Vec<Uuid>, sqlx::Error> {
    if duplicates.len() <= 1 {
        return Ok(Vec::new());
    }

    // Sort by calories (descending) and then by created_at (ascending for tie-breaking)
    let mut sorted_workouts = duplicates;
    sorted_workouts.sort_by(|a, b| {
        // First compare by calories (higher is better)
        let calorie_cmp = b.calories_burned.unwrap_or(0)
            .cmp(&a.calories_burned.unwrap_or(0));

        // If calories are equal, keep the older one (earlier created_at)
        if calorie_cmp == std::cmp::Ordering::Equal {
            a.created_at.cmp(&b.created_at)
        } else {
            calorie_cmp
        }
    });

    // Keep the first one (highest calories or earliest if tied)
    let keeper = &sorted_workouts[0];
    let to_remove: Vec<Uuid> = sorted_workouts
        .iter()
        .skip(1)
        .map(|w| w.id)
        .collect();

    tracing::info!(
        "Keeping workout {} with {} calories, removing {} duplicates",
        keeper.id,
        keeper.calories_burned.unwrap_or(0),
        to_remove.len()
    );

    // Delete the duplicates
    // Note: Since we now calculate stats AFTER cleanup, we don't need to:
    // - Reverse any user stats (they haven't been applied yet)
    // - Delete game events (they haven't been created yet)
    for workout_id in &to_remove {
        sqlx::query!(
            r#"
            DELETE FROM workout_data
            WHERE id = $1
            "#,
            workout_id
        )
        .execute(pool)
        .await?;

        tracing::debug!("Deleted duplicate workout {}", workout_id);
    }

    Ok(to_remove)
}

/// Clean up all duplicate workouts for a user
/// This is the main entry point for the cleanup process
#[tracing::instrument(
    name = "Clean up duplicate workouts for user",
    skip(pool),
    fields(
        user_id = %user_id
    )
)]
pub async fn cleanup_duplicate_workouts_for_user(
    pool: &Pool<Postgres>,
    user_id: Uuid,
) -> Result<usize, Box<dyn std::error::Error>> {
    tracing::info!("Starting duplicate workout cleanup for user {}", user_id);

    // Find all groups of overlapping workouts
    let overlap_groups = find_overlapping_workouts(pool, user_id).await?;

    if overlap_groups.is_empty() {
        tracing::info!("No overlapping workouts found for user {}", user_id);
        return Ok(0);
    }

    let mut total_removed = 0;

    // Process each group of overlapping workouts
    for group in overlap_groups {
        tracing::debug!(
            "Processing group of {} overlapping workouts",
            group.len()
        );

        // Check if workouts have exactly the same start and end times
        let exact_duplicates = find_exact_time_duplicates(&group);

        if !exact_duplicates.is_empty() {
            // Remove duplicates from exact matches
            let removed = remove_duplicate_workouts(pool, exact_duplicates).await?;
            total_removed += removed.len();
        }
    }

    tracing::info!(
        "Completed duplicate cleanup for user {}, removed {} workouts",
        user_id,
        total_removed
    );

    Ok(total_removed)
}

/// Find workouts with exactly the same start and end times
fn find_exact_time_duplicates(workouts: &[OverlappingWorkout]) -> Vec<OverlappingWorkout> {
    let mut exact_duplicates = Vec::new();

    for i in 0..workouts.len() {
        for j in i + 1..workouts.len() {
            // Check if start and end times match exactly
            if workouts[i].workout_start == workouts[j].workout_start
                && workouts[i].workout_end == workouts[j].workout_end
            {
                // Add both to duplicates if not already added
                if !exact_duplicates.iter().any(|w: &OverlappingWorkout| w.id == workouts[i].id) {
                    exact_duplicates.push(workouts[i].clone());
                }
                if !exact_duplicates.iter().any(|w: &OverlappingWorkout| w.id == workouts[j].id) {
                    exact_duplicates.push(workouts[j].clone());
                }
            }
        }
    }

    exact_duplicates
}

/// Run cleanup for all users (can be used in a scheduled job)
#[tracing::instrument(
    name = "Clean up duplicate workouts for all users",
    skip(pool)
)]
pub async fn cleanup_duplicate_workouts_all_users(
    pool: &Pool<Postgres>,
) -> Result<usize, Box<dyn std::error::Error>> {
    tracing::info!("Starting duplicate workout cleanup for all users");

    // Get all users with workouts
    let users = sqlx::query!(
        r#"
        SELECT DISTINCT user_id
        FROM workout_data
        WHERE workout_start IS NOT NULL
            AND workout_end IS NOT NULL
        "#
    )
    .fetch_all(pool)
    .await?;

    let mut total_removed = 0;

    for user_record in users {
        let removed = cleanup_duplicate_workouts_for_user(pool, user_record.user_id).await?;
        total_removed += removed;
    }

    tracing::info!(
        "Completed duplicate cleanup for all users, removed {} workouts total",
        total_removed
    );

    Ok(total_removed)
}

// Make OverlappingWorkout cloneable for the implementation
impl Clone for OverlappingWorkout {
    fn clone(&self) -> Self {
        OverlappingWorkout {
            id: self.id,
            workout_start: self.workout_start,
            workout_end: self.workout_end,
            calories_burned: self.calories_burned,
            device_id: self.device_id.clone(),
            created_at: self.created_at,
        }
    }
}