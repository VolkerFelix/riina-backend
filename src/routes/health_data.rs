use actix_web::{post, web, HttpResponse};
use crate::handlers::health_data::upload_health_data::upload_health_data;
use crate::middleware::auth::Claims;
use crate::models::health_data::HealthDataSyncRequest;
use crate::services::live_game_service::LiveGameService;

#[post("/upload_health")]
async fn upload_health(
    data: web::Json<HealthDataSyncRequest>,
    pool: web::Data<sqlx::PgPool>,
    redis: Option<web::Data<redis::Client>>,
    live_game_service: Option<web::Data<LiveGameService>>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    upload_health_data(data, pool, redis, live_game_service, claims).await
}