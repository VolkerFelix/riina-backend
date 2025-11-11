use actix_web::{web, post};
use crate::handlers::analytics_handler;

#[post("/events")]
pub async fn store_events(
    request: web::Json<crate::models::analytics::AnalyticsEventsRequest>,
    pool: web::Data<sqlx::PgPool>,
) -> actix_web::HttpResponse {
    analytics_handler::store_analytics_events(request, pool).await
}

pub fn init_analytics_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(store_events);
}
