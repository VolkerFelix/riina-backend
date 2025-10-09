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
    
    let results = sqlx::query!(
        r#"
        SELECT stamina_gained, strength_gained
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
    
    let total_points: f32 = results.iter()
        .map(|row| row.stamina_gained + row.strength_gained)
        .sum();
    
    let average = total_points / results.len() as f32;
    Ok(average)
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
            stamina_gained,
            strength_gained
        FROM workout_data
        WHERE user_id = ANY($1) 
        AND workout_start >= $2
        "#,
        user_ids,
        cutoff_date
    )
    .fetch_all(pool)
    .await?;
    
    let mut user_workouts: HashMap<Uuid, Vec<(f32, f32)>> = HashMap::new();
    
    // Group workouts by user
    for row in results {
        let user_id = row.user_id;
        let stamina = row.stamina_gained;
        let strength = row.strength_gained;
        user_workouts.entry(user_id).or_insert_with(Vec::new).push((stamina, strength));
    }
    
    let mut averages = HashMap::new();
    
    // Calculate average for each user
    for user_id in user_ids {
        if let Some(workouts) = user_workouts.get(user_id) {
            let total_points: f32 = workouts.iter()
                .map(|(stamina, strength)| stamina + strength)
                .sum();
            let average = total_points / workouts.len() as f32;
            averages.insert(*user_id, average);
        } else {
            averages.insert(*user_id, 0.0);
        }
    }
    
    Ok(averages)
}
