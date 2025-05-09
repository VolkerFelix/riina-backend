// src/handlers/auth_handler.rs
use actix_web::{web, HttpResponse};
use secrecy::ExposeSecret;
use sqlx::PgPool;
use chrono::{Utc, Duration};
use jsonwebtoken::{encode, EncodingKey, Header};

use crate::models::auth::{LoginRequest, LoginResponse};
use crate::utils::password::verify_password;
use crate::config::jwt::JwtSettings;

#[derive(serde::Serialize, serde::Deserialize)]
struct Claims {
    sub: String,  // Subject (user id)
    username: String,
    exp: usize,   // Expiration time (as UTC timestamp)
}

#[tracing::instrument(
    name = "Login user attempt",
    skip(login_form, pool, jwt_settings),
    fields(
        username = %login_form.username
    )
)]
pub async fn login_user(
    login_form: web::Json<LoginRequest>,
    pool: web::Data<PgPool>,
    jwt_settings: web::Data<JwtSettings>
) -> HttpResponse {
    let user_result = sqlx::query!(
        r#"
        SELECT id, username, password_hash
        FROM users
        WHERE username = $1
        "#,
        login_form.username,
    )
    .fetch_optional(pool.get_ref())
    .await;

    // Return database error to user as 500
    let user = match user_result {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::info!("User not found or invalid credentials");
            return HttpResponse::Unauthorized().finish();
        }
        Err(e) => {
            tracing::error!("Database error occurred: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    // Verify password
    if !verify_password(
        login_form.password.expose_secret(),
        &user.password_hash
    ) {
        tracing::info!("Invalid password");
        return HttpResponse::Unauthorized().finish();
    }

    // Generate JWT token
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(24))
        .expect("Valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user.id.to_string(),
        username: user.username,
        exp: expiration,
    };

    let token = match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_settings.secret.expose_secret().as_bytes()),
    ) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("Error generating JWT token: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    // Return token
    HttpResponse::Ok().json(LoginResponse { token })
}