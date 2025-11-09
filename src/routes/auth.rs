// src/routes/auth.rs
use actix_web::{post, web, HttpResponse};
use sqlx::PgPool;

use crate::handlers::auth_handler::{login_user, refresh_biometric_token, reset_password};
use crate::models::auth::{LoginRequest, BiometricRefreshRequest, ResetPasswordRequest};
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

#[post("/reset-password")]
async fn reset_password_route(
    reset_form: web::Json<ResetPasswordRequest>,
    pool: web::Data<PgPool>,
) -> HttpResponse {
    reset_password(reset_form, pool).await
}