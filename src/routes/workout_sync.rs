use actix_web::{web, get, post, HttpResponse};
use sqlx::PgPool;
use crate::middleware::auth::Claims;
use crate::handlers::workout_data::activity::{get_activity_summary, get_zone_analysis};
use crate::handlers::workout_data::workout_history::get_workout_history;
use crate::handlers::workout_data::check_workout_sync::{check_workout_sync, CheckSyncStatusRequest};
use crate::config::jwt::JwtSettings;

#[get("/activity")]
async fn get_activity_sum(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    get_activity_summary(pool, claims).await
}

#[get("/zones")]
async fn get_zone_ana(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    get_zone_analysis(pool, claims).await
}

#[get("/history")]
async fn get_workout_hist(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    query: web::Query<crate::handlers::workout_data::workout_history::WorkoutHistoryQuery>
) -> HttpResponse {
    get_workout_history(pool, claims, query).await
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