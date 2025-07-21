use actix_web::{web, get, HttpResponse};
use sqlx::PgPool;
use crate::middleware::auth::Claims;
use crate::handlers::health_data::activity::{get_activity_summary, get_zone_analysis};
use crate::handlers::health_data::workout_history::get_workout_history;

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
    query: web::Query<crate::handlers::health_data::workout_history::WorkoutHistoryQuery>
) -> HttpResponse {
    get_workout_history(pool, claims, query).await
}