use sqlx::PgPool;
use uuid::Uuid;
use chrono::{Utc, Duration};

/// Calculate the trailing 7-day average of workout points for a user
/// Returns the average of (stamina_gained + strength_gained) over the last 7 days
#[tracing::instrument(
    name = "Calculate trailing 7-day average",
    skip(pool),
    fields(user_id = %user_id)
)]
pub async fn calculate_trailing_7_day_average(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<f32, sqlx::Error> {
    let seven_days_ago = Utc::now() - Duration::days(7);
    
    let result = sqlx::query!(
        r#"
        SELECT AVG(stamina_gained + strength_gained) as avg_points
        FROM workout_data
        WHERE user_id = $1 
        AND workout_start >= $2
        "#,
        user_id,
        seven_days_ago
    )
    .fetch_one(pool)
    .await?;
    
    Ok(result.avg_points.unwrap_or(0.0) as f32)
}

/// Calculate trailing 7-day averages for multiple users in batch
/// Returns a HashMap mapping user_id to their trailing 7-day average
#[tracing::instrument(
    name = "Calculate trailing 7-day averages for multiple users",
    skip(pool),
    fields(user_count = user_ids.len())
)]
pub async fn calculate_trailing_7_day_averages_batch(
    pool: &PgPool,
    user_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, f32>, sqlx::Error> {
    if user_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    
    let seven_days_ago = Utc::now() - Duration::days(7);
    
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
        seven_days_ago
    )
    .fetch_all(pool)
    .await?;
    
    let mut averages = std::collections::HashMap::new();
    for row in results {
        let user_id = row.user_id;
        let avg_points = row.avg_points.unwrap_or(0.0) as f32;
        averages.insert(user_id, avg_points);
    }
    
    // Fill in 0.0 for users with no workouts in the last 7 days
    for user_id in user_ids {
        averages.entry(*user_id).or_insert(0.0);
    }
    
    Ok(averages)
}
