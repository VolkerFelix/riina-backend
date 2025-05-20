use actix_web::{web, HttpResponse};
use uuid::Uuid;
use sqlx::PgPool;
use serde_json::json;

use crate::middleware::auth::Claims;

#[tracing::instrument(
    name = "Get user health state",
    skip(pool, claims),
    fields(
        username = %claims.username
    )
)]
pub async fn get_user_health_state(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    tracing::info!("Getting health state for user: {}", claims.username);
    
    // Parse user ID from claims
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": "Invalid user ID"
            }));
        }
    };
    
    // Query all user states from the database
    let user_states = match get_all_user_states(&pool, &user_id).await {
        Ok(states) => states,
        Err(e) => {
            tracing::error!("Failed to get user states: {}", e);
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": "Failed to get user health state"
            }));
        }
    };
    
    // Return the user states
    HttpResponse::Ok().json(json!({
        "status": "success",
        "data": user_states
    }))
}

async fn get_all_user_states(pool: &PgPool, user_id: &Uuid) -> Result<serde_json::Value, sqlx::Error> {
    // Query for all the user states (sleep, twin, etc.)
    let rows = sqlx::query!(
        r#"
        SELECT state_type, state_value, last_updated
        FROM user_states
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    
    // Convert to a map of state_type -> state_value
    let mut states = serde_json::Map::new();
    for row in rows {
        states.insert(row.state_type, row.state_value);
    }
    
    // Also include the latest health data as context
    let latest_health_data = sqlx::query!(
        r#"
        SELECT id, timestamp, steps, heart_rate, sleep, active_energy_burned, additional_metrics
        FROM health_data
        WHERE user_id = $1
        ORDER BY timestamp DESC
        LIMIT 1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    
    if let Some(data) = latest_health_data {
        let health_data_json = json!({
            "id": data.id,
            "timestamp": data.timestamp,
            "steps": data.steps,
            "heart_rate": data.heart_rate,
            "sleep": data.sleep,
            "active_energy_burned": data.active_energy_burned,
            "additional_metrics": data.additional_metrics
        });
        
        states.insert("latest_health_data".to_string(), health_data_json);
    }
    
    Ok(serde_json::Value::Object(states))
}