use sqlx::{Pool, Postgres, Error};
use uuid::Uuid;

pub async fn get_heart_rate_reserve(pool: &Pool<Postgres>, user_id: Uuid) -> Result<i16, Error> {
    let (resting_heart_rate, max_heart_rate) = sqlx::query!(
        r#"
        SELECT resting_heart_rate, max_heart_rate 
        FROM user_health_profiles 
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(&pool)
    .await?
    .map(|row| (row.resting_heart_rate, row.max_heart_rate))
    .ok_or(Error::RowNotFound)?;

    Ok(max_heart_rate - resting_heart_rate)
}

// Get heart rate zones
pub fn get_heart_rate_zones(hhr: i16) -> i16 {
    let percentage = (heart_rate / hhr as f32 * 100.0) as i16;
    percentage
}