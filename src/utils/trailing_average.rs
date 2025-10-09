use sqlx::PgPool;
use uuid::Uuid;
use chrono::{Utc, Duration};
use std::collections::HashMap;

/// Number of days for trailing average calculation
/// This can be easily changed to make the trailing period configurable
pub const TRAILING_AVERAGE_DAYS: i64 = 7;

/// Calculate the trailing average of workout points for a user
/// Returns the average of (stamina_gained + strength_gained) over the configured trailing period
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
    
    let result = sqlx::query!(
        r#"
        SELECT AVG(stamina_gained + strength_gained) as avg_points
        FROM workout_data
        WHERE user_id = $1 
        AND workout_start >= $2
        "#,
        user_id,
        cutoff_date
    )
    .fetch_one(pool)
    .await?;
    
    Ok(result.avg_points.unwrap_or(0.0) as f32)
}

/// Calculate trailing averages for multiple users in batch
/// Returns a HashMap mapping user_id to their trailing average
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
            AVG(stamina_gained + strength_gained) as avg_points
        FROM workout_data
        WHERE user_id = ANY($1) 
        AND workout_start >= $2
        GROUP BY user_id
        "#,
        user_ids,
        cutoff_date
    )
    .fetch_all(pool)
    .await?;
    
    let mut averages = HashMap::new();
    for row in results {
        let user_id = row.user_id;
        let avg_points = row.avg_points.unwrap_or(0.0) as f32;
        averages.insert(user_id, avg_points);
    }
    
    // Fill in 0.0 for users with no workouts in the trailing period
    for user_id in user_ids {
        averages.entry(*user_id).or_insert(0.0);
    }
    
    Ok(averages)
}
