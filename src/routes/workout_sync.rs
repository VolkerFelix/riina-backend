use actix_web::{web, get, post, delete, HttpResponse};
use sqlx::PgPool;
use crate::middleware::auth::Claims;
use crate::handlers::workout_data::workout_history::get_workout_history;
use crate::handlers::workout_data::workout_detail::get_workout_detail;
use crate::handlers::workout_data::check_workout_sync::{check_workout_sync, CheckSyncStatusRequest};
use crate::handlers::workout_data::scoring_feedback::{submit_scoring_feedback, get_scoring_feedback};
use crate::handlers::workout_data::workout_reports::{
    submit_workout_report, get_my_report_for_workout, get_my_reports, delete_workout_report
};
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

#[post("/workout/{workout_id}/scoring-feedback")]
async fn submit_scoring_feedback_handler(
    pool: web::Data<PgPool>,
    workout_id: web::Path<uuid::Uuid>,
    claims: web::ReqData<Claims>,
    request: web::Json<crate::models::workout_data::SubmitScoringFeedbackRequest>,
) -> actix_web::Result<HttpResponse> {
    submit_scoring_feedback(pool, workout_id, claims, request).await
}

#[get("/workout/{workout_id}/scoring-feedback")]
async fn get_scoring_feedback_handler(
    pool: web::Data<PgPool>,
    workout_id: web::Path<uuid::Uuid>,
    claims: web::ReqData<Claims>,
) -> actix_web::Result<HttpResponse> {
    get_scoring_feedback(pool, workout_id, claims).await
}

#[post("/workout/{workout_id}/report")]
async fn submit_workout_report_handler(
    pool: web::Data<PgPool>,
    workout_id: web::Path<uuid::Uuid>,
    claims: web::ReqData<Claims>,
    request: web::Json<crate::models::workout_data::SubmitWorkoutReportRequest>,
) -> actix_web::Result<HttpResponse> {
    submit_workout_report(pool, workout_id, claims, request).await
}

#[get("/workout/{workout_id}/my-report")]
async fn get_my_report_for_workout_handler(
    pool: web::Data<PgPool>,
    workout_id: web::Path<uuid::Uuid>,
    claims: web::ReqData<Claims>,
) -> actix_web::Result<HttpResponse> {
    get_my_report_for_workout(pool, workout_id, claims).await
}

#[get("/reports/my-reports")]
async fn get_my_reports_handler(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> actix_web::Result<HttpResponse> {
    get_my_reports(pool, claims).await
}

#[delete("/reports/{report_id}")]
async fn delete_workout_report_handler(
    pool: web::Data<PgPool>,
    report_id: web::Path<uuid::Uuid>,
    claims: web::ReqData<Claims>,
) -> actix_web::Result<HttpResponse> {
    delete_workout_report(pool, report_id, claims).await
}