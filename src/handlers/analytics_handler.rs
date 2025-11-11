// src/handlers/analytics_handler.rs
use actix_web::{web, HttpResponse};
use sqlx::{PgPool, types::Json};
use crate::models::analytics::{AnalyticsEventsRequest, EventData};

#[tracing::instrument(
    name = "Store analytics events",
    skip(request, pool),
    fields(
        event_count = %request.events.len()
    )
)]
pub async fn store_analytics_events(
    request: web::Json<AnalyticsEventsRequest>,
    pool: web::Data<PgPool>,
) -> HttpResponse {
    // Validate all events first
    for (idx, event) in request.events.iter().enumerate() {
        if let Err(e) = event.validate() {
            tracing::warn!(
                event_index = idx,
                event_name = %event.event_name,
                error = e,
                "Invalid event data"
            );
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid event at index {}: {}", idx, e)
            }));
        }
    }

    // Insert events in batch
    let mut inserted_count = 0;

    for event in &request.events {
        let timestamp = event.get_timestamp();
        let event_data_json: Option<Json<EventData>> = event.event_data.as_ref().map(|data| Json(data.clone()));

        let result = sqlx::query!(
            r#"
            INSERT INTO analytics_events (
                event_name,
                event_data,
                screen_name,
                session_id,
                user_hash,
                timestamp,
                platform
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            event.event_name,
            event_data_json as Option<Json<EventData>>,
            event.screen_name.as_deref(),
            event.session_id.as_deref(),
            event.user_hash.as_deref(),
            timestamp,
            event.platform
        )
        .execute(pool.get_ref())
        .await;

        match result {
            Ok(_) => {
                inserted_count += 1;
            }
            Err(e) => {
                tracing::error!(
                    event_name = %event.event_name,
                    error = ?e,
                    "Failed to insert analytics event"
                );
            }
        }
    }

    tracing::info!(
        total_events = %request.events.len(),
        inserted_events = %inserted_count,
        "Analytics events stored"
    );

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "inserted": inserted_count,
        "total": request.events.len()
    }))
}
