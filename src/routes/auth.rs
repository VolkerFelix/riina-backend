// src/routes/auth.rs
use actix_web::{post, web, HttpResponse};
use sqlx::PgPool;

use crate::handlers::auth_handler::login_user;
use crate::models::auth::LoginRequest;
use crate::config::jwt::JwtSettings;

#[post("/login")]
async fn login(
    login_form: web::Json<LoginRequest>, 
    pool: web::Data<PgPool>,
    jwt_settings: web::Data<JwtSettings>
) -> HttpResponse {
    login_user(login_form, pool, jwt_settings).await
}