// src/handlers/auth_handler.rs
use actix_web::{web, HttpResponse};
use secrecy::ExposeSecret;
use sqlx::PgPool;
use chrono::{Utc, Duration};
use jsonwebtoken::{encode, EncodingKey, Header};

use crate::models::auth::{LoginRequest, LoginResponse, BiometricRefreshRequest, ResetPasswordRequest};
use crate::models::user::{UserRole, UserStatus};
use crate::utils::password::{verify_password, hash_password};
use crate::config::jwt::JwtSettings;
use crate::middleware::auth::Claims;

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
        SELECT id, username, password_hash, role, status
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

    let role = match user.role.as_str() {
        "superadmin" => UserRole::SuperAdmin,
        "admin" => UserRole::Admin,
        "moderator" => UserRole::Moderator,
        _ => UserRole::User,
    };
    
    let status = match user.status.as_str() {
        "inactive" => UserStatus::Inactive,
        "suspended" => UserStatus::Suspended,
        "banned" => UserStatus::Banned,
        _ => UserStatus::Active,
    };
    
    let claims = Claims {
        sub: user.id.to_string(),
        username: user.username,
        role,
        status,
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

use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};

#[tracing::instrument(
    name = "Refresh biometric token",
    skip(refresh_request, pool, jwt_settings),
)]
pub async fn refresh_biometric_token(
    refresh_request: web::Json<BiometricRefreshRequest>,
    pool: web::Data<PgPool>,
    jwt_settings: web::Data<JwtSettings>,
) -> HttpResponse {
    let expired_token = &refresh_request.token;
    // Decode the expired token with validation disabled for expiry
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = false; // Don't validate expiry
    validation.validate_nbf = false;

    let token_data = match decode::<Claims>(
        &expired_token,
        &DecodingKey::from_secret(jwt_settings.secret.expose_secret().as_bytes()),
        &validation,
    ) {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!("Invalid token provided for refresh: {:?}", e);
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "Invalid token"
            }));
        }
    };

    let claims = token_data.claims;

    // Check if token is not too old (e.g., expired less than 30 days ago)
    let now = Utc::now().timestamp() as usize;
    let thirty_days_in_seconds = 30 * 24 * 60 * 60;

    if claims.exp + thirty_days_in_seconds < now {
        tracing::warn!("Token too old for refresh (expired more than 30 days ago)");
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Token too old for refresh"
        }));
    }

    let Some(user_id) = claims.user_id() else {
        tracing::error!("Invalid user ID in claims");
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid user ID"
        }));
    };

    // Verify user still exists and is active
    let user_result = sqlx::query!(
        r#"
        SELECT id, username, role, status
        FROM users
        WHERE id = $1 AND status = 'active'
        "#,
        user_id,
    )
    .fetch_optional(pool.get_ref())
    .await;

    let user = match user_result {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::warn!("User not found or inactive for token refresh");
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "User not found or inactive"
            }));
        }
        Err(e) => {
            tracing::error!("Database error during token refresh: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    // Generate new JWT token with fresh expiry
    let new_expiration = Utc::now()
        .checked_add_signed(Duration::hours(24))
        .expect("Valid timestamp")
        .timestamp() as usize;

    let role = match user.role.as_str() {
        "superadmin" => UserRole::SuperAdmin,
        "admin" => UserRole::Admin,
        "moderator" => UserRole::Moderator,
        _ => UserRole::User,
    };

    let status = match user.status.as_str() {
        "inactive" => UserStatus::Inactive,
        "suspended" => UserStatus::Suspended,
        "banned" => UserStatus::Banned,
        _ => UserStatus::Active,
    };

    let new_claims = Claims {
        sub: user.id.to_string(),
        username: user.username,
        role,
        status,
        exp: new_expiration,
    };

    let new_token = match encode(
        &Header::default(),
        &new_claims,
        &EncodingKey::from_secret(jwt_settings.secret.expose_secret().as_bytes()),
    ) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("Error generating new JWT token: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    tracing::info!("Successfully refreshed token for user {}", new_claims.sub);
    HttpResponse::Ok().json(LoginResponse { token: new_token })
}

#[tracing::instrument(
    name = "Reset password",
    skip(reset_request, pool),
    fields(
        username = %reset_request.username
    )
)]
pub async fn reset_password(
    reset_request: web::Json<ResetPasswordRequest>,
    pool: web::Data<PgPool>,
) -> HttpResponse {
    // Check if user exists
    let user_result = sqlx::query!(
        r#"
        SELECT id
        FROM users
        WHERE username = $1
        "#,
        reset_request.username,
    )
    .fetch_optional(pool.get_ref())
    .await;

    let user = match user_result {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::info!("User not found for password reset");
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": "User not found"
            }));
        }
        Err(e) => {
            tracing::error!("Database error during password reset: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    // Hash the new password
    let password_hash = hash_password(reset_request.new_password.expose_secret());

    // Update the password in the database
    let update_result = sqlx::query!(
        r#"
        UPDATE users
        SET password_hash = $1
        WHERE id = $2
        "#,
        password_hash,
        user.id,
    )
    .execute(pool.get_ref())
    .await;

    match update_result {
        Ok(_) => {
            tracing::info!("Password reset successful for user {}", reset_request.username);
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "Password reset successful"
            }))
        }
        Err(e) => {
            tracing::error!("Error updating password: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}