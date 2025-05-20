use sqlx::{Pool, Postgres, Error};
use uuid::Uuid;
use serde_json::Value as JsonValue;

use crate::models::health_data::HealthDataSyncRequest;

pub async fn insert_health_data(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    data: &HealthDataSyncRequest,
) -> Result<Uuid, sqlx::Error> {
    // Convert SleepData to Json if present
    let sleep_json = match &data.sleep {
        Some(sleep) => serde_json::to_value(sleep).map_err(|e| {
            tracing::error!("Failed to serialize SleepData to Json: {}", e);
            Error::Protocol("Failed to serialize SleepData".into())
        })?,
        None => JsonValue::Null,
    };
    
    // Convert additional_metrics to Json if present
    let additional_metrics_json = match &data.additional_metrics {
        Some(metrics) => serde_json::to_value(metrics).map_err(|e| {
            tracing::error!("Failed to serialize additional_metrics to Json: {}", e);
            Error::Protocol("Failed to serialize additional_metrics".into())
        })?,
        None => JsonValue::Null,
    };
    
    let record = sqlx::query!(
        r#"
        INSERT INTO health_data (
            user_id, device_id, timestamp, steps, heart_rate, 
            sleep, active_energy_burned, additional_metrics
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#,
        user_id,
        &data.device_id,
        data.timestamp,
        data.steps,
        data.heart_rate,
        sleep_json,
        data.active_energy_burned,
        additional_metrics_json
    )
    .fetch_one(pool)
    .await?;
    
    Ok(record.id)
}