use actix_web::{get, post, web, HttpResponse};
use crate::handlers::health_data::get_user_health_state::get_user_health_state;
use crate::handlers::health_data::upload_health_data::upload_health_data;
use crate::middleware::auth::Claims;
use crate::models::health_data::HealthDataSyncRequest;

#[post("/upload_health")]
async fn upload_health(
    data: web::Json<HealthDataSyncRequest>,
    pool: web::Data<sqlx::PgPool>,
    redis: Option<web::Data<redis::Client>>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    upload_health_data(data, pool, redis, claims).await
}

#[get("/state")]
async fn get_state(
    pool: web::Data<sqlx::PgPool>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    get_user_health_state(pool, claims).await
}