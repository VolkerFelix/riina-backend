use actix_web::{web, get, post, HttpResponse};
use sqlx::PgPool;
use crate::middleware::auth::Claims;
use crate::handlers::workout_data::workout_history::get_workout_history;
use crate::handlers::workout_data::workout_detail::get_workout_detail;
use crate::handlers::workout_data::check_workout_sync::{check_workout_sync, CheckSyncStatusRequest};
use crate::config::jwt::JwtSettings;

#[get("/history")]
async fn get_workout_hist(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    query: web::Query<crate::handlers::workout_data::workout_history::WorkoutHistoryQuery>
) -> HttpResponse {
    get_workout_history(pool, claims, query).await
}

#[get("/workout/{id}")]
async fn get_workout_detail_handler(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    workout_id: web::Path<uuid::Uuid>,
) -> HttpResponse {
    get_workout_detail(pool, claims, workout_id).await
}

#[post("/check_sync_status")]
async fn check_workout_sync_handler(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    request: web::Json<CheckSyncStatusRequest>,
    jwt_settings: web::Data<JwtSettings>,
) -> HttpResponse {
    check_workout_sync(pool, claims, request, jwt_settings).await
}