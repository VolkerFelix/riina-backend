use sqlx::PgPool;
use uuid::Uuid;
use chrono::{Utc, Duration};
use std::collections::HashMap;

/// Number of days for trailing average calculation
/// This can be easily changed to make the trailing period configurable
pub const TRAILING_AVERAGE_DAYS: i64 = 7;

/// Calculate the trailing average of workout points for a user
/// Returns the average of (stamina_gained + strength_gained) over the last 7 days
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

    // Sum all workout points from the period
    let total_points: f32 = results
        .iter()
        .map(|row| row.stamina_gained + row.strength_gained)
        .sum();

    let average = total_points / TRAILING_AVERAGE_DAYS as f32;
    Ok(average)
}

/// Calculate trailing averages for multiple users in batch
/// Returns a HashMap mapping user_id to their trailing average over the last 7 days
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

    // Sum workout points per user
    let mut user_totals: HashMap<Uuid, f32> = HashMap::new();

    for row in results {
        let score = row.stamina_gained + row.strength_gained;
        *user_totals.entry(row.user_id).or_insert(0.0) += score;
    }

    // Calculate average for each user
    let averages: HashMap<Uuid, f32> = user_ids
        .iter()
        .map(|user_id| {
            let total = user_totals.get(user_id).copied().unwrap_or(0.0);
            (*user_id, total / TRAILING_AVERAGE_DAYS as f32)
        })
        .collect();

    Ok(averages)
}
