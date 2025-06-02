use actix_web::{post, web, HttpResponse};
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