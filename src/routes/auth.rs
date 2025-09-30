// src/routes/auth.rs
use actix_web::{post, web, HttpResponse};
use sqlx::PgPool;

use crate::handlers::auth_handler::{login_user, refresh_biometric_token};
use crate::models::auth::{LoginRequest, BiometricRefreshRequest};
use crate::config::jwt::JwtSettings;

#[post("/login")]
async fn login(
    login_form: web::Json<LoginRequest>,
    pool: web::Data<PgPool>,
    jwt_settings: web::Data<JwtSettings>
) -> HttpResponse {
    login_user(login_form, pool, jwt_settings).await
}

#[post("/biometric-refresh")]
async fn biometric_refresh(
    refresh_form: web::Json<BiometricRefreshRequest>,
    pool: web::Data<PgPool>,
    jwt_settings: web::Data<JwtSettings>
) -> HttpResponse {
    refresh_biometric_token(refresh_form, pool, jwt_settings).await
}