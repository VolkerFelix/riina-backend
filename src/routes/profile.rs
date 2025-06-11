use actix_web::{web, get, put, HttpResponse};
use sqlx::PgPool;
use crate::handlers::profile::profile::get_user_profile;
use crate::handlers::profile::health_profile::{get_health_profile, update_health_profile};
use crate::middleware::auth::Claims;
use crate::models::profile::UpdateHealthProfileRequest;

#[get("/user")]
async fn get_user(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    get_user_profile(pool, claims).await
}

#[get("/health_profile")]
async fn get_health_prof(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    get_health_profile(pool, claims).await
}

#[put("/health_profile")]
async fn update_health_prof(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    data: web::Json<UpdateHealthProfileRequest>,
) -> HttpResponse {
    update_health_profile(pool, claims, data).await
}