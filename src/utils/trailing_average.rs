use sqlx::PgPool;
use uuid::Uuid;
use chrono::{Utc, Duration};
use std::collections::HashMap;

/// Number of days for trailing average calculation
/// This can be easily changed to make the trailing period configurable
pub const TRAILING_AVERAGE_DAYS: i64 = 7;

/// Number of best days to use for trailing average calculation
/// We take the best 5 out of 7 days, discarding the 2 worst days
pub const TRAILING_AVERAGE_BEST_DAYS: usize = 5;

/// Calculate the trailing average of workout points for a user
/// Returns the average of (stamina_gained + strength_gained) over the configured trailing period
/// Uses the best 5 out of 7 days, discarding the 2 worst days
#[tracing::instrument(
    name = "Calculate trailing average",
    skip(pool),
    fields(user_id = %user_id)
)]
pub async fn calculate_trailing_average(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<f32, sqlx::Error> {
    let cutoff_date = Utc::now() - Duration::days(TRAILING_AVERAGE_DAYS);

    let results = sqlx::query!(
        r#"
        SELECT stamina_gained, strength_gained, DATE(workout_start) as workout_date
        FROM workout_data
        WHERE user_id = $1
        AND workout_start >= $2
        "#,
        user_id,
        cutoff_date
    )
    .fetch_all(pool)
    .await?;

    if results.is_empty() {
        return Ok(0.0);
    }

    // Group workouts by day and calculate daily totals
    let mut daily_scores: HashMap<chrono::NaiveDate, f32> = HashMap::new();

    for row in results {
        let daily_score = row.stamina_gained + row.strength_gained;
        let workout_date = row.workout_date.unwrap_or_else(|| chrono::Utc::now().date_naive());

        *daily_scores.entry(workout_date).or_insert(0.0) += daily_score;
    }

    // Convert to vector and sort by score (descending)
    let mut daily_scores_vec: Vec<f32> = daily_scores.into_values().collect();
    daily_scores_vec.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    // Take the best days up to TRAILING_AVERAGE_BEST_DAYS
    let best_days_count = daily_scores_vec.len().min(TRAILING_AVERAGE_BEST_DAYS);
    let total_points: f32 = daily_scores_vec.iter().take(best_days_count).sum();

    let average = total_points / TRAILING_AVERAGE_DAYS as f32;
    Ok(average)
}

/// Calculate trailing averages for multiple users in batch
/// Returns a HashMap mapping user_id to their trailing average
/// Uses the best 5 out of 7 days, discarding the 2 worst days
#[tracing::instrument(
    name = "Calculate trailing averages for multiple users",
    skip(pool),
    fields(user_count = user_ids.len())
)]
pub async fn calculate_trailing_averages_batch(
    pool: &PgPool,
    user_ids: &[Uuid],
) -> Result<HashMap<Uuid, f32>, sqlx::Error> {
    if user_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let cutoff_date = Utc::now() - Duration::days(TRAILING_AVERAGE_DAYS);

    let results = sqlx::query!(
        r#"
        SELECT
            user_id,
            stamina_gained,
            strength_gained,
            DATE(workout_start) as workout_date
        FROM workout_data
        WHERE user_id = ANY($1)
        AND workout_start >= $2
        "#,
        user_ids,
        cutoff_date
    )
    .fetch_all(pool)
    .await?;

    // Group workouts by user, then by day
    let mut user_daily_scores: HashMap<Uuid, HashMap<chrono::NaiveDate, f32>> = HashMap::new();

    for row in results {
        let user_id = row.user_id;
        let daily_score = row.stamina_gained + row.strength_gained;
        let workout_date = row.workout_date.unwrap_or_else(|| chrono::Utc::now().date_naive());

        user_daily_scores
            .entry(user_id)
            .or_insert_with(HashMap::new)
            .entry(workout_date)
            .and_modify(|score| *score += daily_score)
            .or_insert(daily_score);
    }

    let mut averages = HashMap::new();

    // Calculate average for each user using best days
    for user_id in user_ids {
        if let Some(daily_scores) = user_daily_scores.get(user_id) {
            // Convert to vector and sort by score (descending)
            let mut daily_scores_vec: Vec<f32> = daily_scores.values().copied().collect();
            daily_scores_vec.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

            // Take the best days up to TRAILING_AVERAGE_BEST_DAYS
            let best_days_count = daily_scores_vec.len().min(TRAILING_AVERAGE_BEST_DAYS);
            let total_points: f32 = daily_scores_vec.iter().take(best_days_count).sum();

            let average = total_points / TRAILING_AVERAGE_DAYS as f32;
            averages.insert(*user_id, average);
        } else {
            averages.insert(*user_id, 0.0);
        }
    }

    Ok(averages)
}
