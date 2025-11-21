use actix_web::{post, web, HttpResponse};
use crate::handlers::workout_data::upload_workout_data::upload_workout_data;
use crate::middleware::auth::Claims;
use crate::models::workout_data::WorkoutDataUploadRequest;
use crate::config::jwt::JwtSettings;
use crate::services::ml_client::MLClient;
use std::sync::Arc;

#[post("/upload_health")]
async fn upload_health(
    data: web::Json<WorkoutDataUploadRequest>,
    pool: web::Data<sqlx::PgPool>,
    redis: Option<web::Data<Arc<redis::Client>>>,
    claims: web::ReqData<Claims>,
    jwt_settings: web::Data<JwtSettings>,
    ml_client: web::Data<MLClient>,
) -> HttpResponse {
    upload_workout_data(data, pool, redis, claims, jwt_settings, ml_client).await
}
