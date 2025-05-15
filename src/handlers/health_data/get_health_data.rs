use actix_web::{web, HttpResponse};
use crate::middleware::auth::Claims;
use crate::models::health_data::{HealthData, SleepData};
use sqlx::{PgPool, types::Json};
use uuid::Uuid;

#[tracing::instrument(
    name = "Get health data",
    skip(pool, claims),
    fields(
        username = %claims.username
    )
)]

pub async fn get_health_data(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    // Parse user_id from claims
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    // Query the database for health data
    let health_data_result = sqlx::query_as!(
        HealthData,
        r#"
        SELECT 
            id,
            user_id,
            device_id,
            timestamp,
            steps,
            heart_rate,
            sleep as "sleep: Json<SleepData>",
            active_energy_burned,
            additional_metrics as "additional_metrics: Json<serde_json::Value>",
            created_at
        FROM health_data
        WHERE user_id = $1
        ORDER BY timestamp DESC
        LIMIT 50
        "#,
        user_id
    )
    .fetch_all(pool.get_ref())
    .await;

    match health_data_result {
        Ok(health_data) => {
            // Convert the raw data to a format suitable for the frontend
            let health_data_json = health_data.iter().map(|record| {
                serde_json::json!({
                    "id": record.id,
                    "device_id": &record.device_id,
                    "timestamp": record.timestamp,
                    "steps": record.steps,
                    "heart_rate": record.heart_rate,
                    "sleep": record.sleep,
                    "active_energy_burned": record.active_energy_burned,
                    "additional_metrics": record.additional_metrics,
                    "created_at": record.created_at
                })
            }).collect::<Vec<_>>();
            tracing::info!("Health data: {:?}", health_data_json);
            HttpResponse::Ok().json(health_data_json)
        },
        Err(e) => {
            tracing::error!("Database error: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}