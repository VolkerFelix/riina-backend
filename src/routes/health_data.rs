use actix_web::{post, web, HttpResponse};
use crate::handlers::workout_data::upload_workout_data::upload_workout_data;
use crate::middleware::auth::Claims;
use crate::models::workout_data::WorkoutDataSyncRequest;
use crate::services::live_game_service::LiveGameService;
use std::sync::Arc;

#[post("/upload_health")]
async fn upload_health(
    data: web::Json<WorkoutDataSyncRequest>,
    pool: web::Data<sqlx::PgPool>,
    redis: Option<web::Data<Arc<redis::Client>>>,
    live_game_service: Option<web::Data<LiveGameService>>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    upload_workout_data(data, pool, redis, live_game_service, claims).await
}