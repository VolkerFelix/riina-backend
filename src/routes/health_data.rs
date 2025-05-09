// Update src/routes/health_data.rs
use actix_web::{post, web, HttpResponse};
use crate::handlers::health_data::upload_health_data::upload_health_data;
use crate::middleware::auth::Claims;
use crate::models::health_data::HealthDataSyncRequest;

#[post("/sync_health")]
async fn sync_health(
    data: web::Json<HealthDataSyncRequest>,
    pool: web::Data<sqlx::PgPool>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    upload_health_data(data, pool, claims).await
}