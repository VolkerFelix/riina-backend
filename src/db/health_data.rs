use sqlx::{Pool, Postgres};
use uuid::Uuid;
use serde_json::json;

use crate::models::health_data::HealthDataSyncRequest;

pub async fn insert_health_data(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    data: &HealthDataSyncRequest,
) -> Result<Uuid, sqlx::Error> {
    let record = sqlx::query!(
        r#"
        INSERT INTO health_data (
            user_id, device_id, heart_rate_data, 
            active_energy_burned
        )
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#,
        user_id,
        &data.device_id,
        json!(data.heart_rate),
        data.active_energy_burned
    )
    .fetch_one(pool)
    .await?;
    
    Ok(record.id)
}