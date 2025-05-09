use sqlx::{Pool, Postgres, Error};
use sqlx::types::Json;
use uuid::Uuid;

use crate::models::health_data::{HealthData, HealthDataSyncRequest, SleepData};

pub async fn insert_health_data(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    data: &HealthDataSyncRequest,
) -> Result<Uuid, sqlx::Error> {
    // Convert SleepData to Json<SleepData> if present
    let sleep_json = match &data.sleep {
        Some(sleep) => match serde_json::to_value(sleep) {
            Ok(json) => json,
            Err(e) => {
                tracing::error!("Failed to serialize SleepData to Json: {}", e);
                return Err(Error::Protocol("Failed to serialize SleepData".into()));
            }
        },
        None => serde_json::Value::Null,
    };
    
    // Convert additional_metrics to Json if present
    let additional_metrics_json = match &data.additional_metrics {
        Some(metrics) => match serde_json::to_value(metrics) {
            Ok(json) => json,
            Err(e) => {
                tracing::error!("Failed to serialize additional_metrics to Json: {}", e);
                return Err(Error::Protocol("Failed to serialize additional_metrics".into()));
            }
        },
        None => serde_json::Value::Null,
    };
    
    let record = sqlx::query_as!(
        HealthData,
        r#"
        INSERT INTO health_data (
            user_id, device_id, timestamp, steps, heart_rate, 
            sleep, active_energy_burned, additional_metrics
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id, user_id, device_id, timestamp, steps, heart_rate, 
                  sleep as "sleep: Json<SleepData>", active_energy_burned,
                  additional_metrics as "additional_metrics: Json<serde_json::Value>", created_at
        "#,
        user_id,
        data.device_id,
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