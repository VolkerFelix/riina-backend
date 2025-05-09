use actix_web::{web, HttpResponse};
use secrecy::ExposeSecret;
use sqlx::PgPool;
use chrono::Utc;
use uuid::Uuid;

use crate::models::user::RegistrationRequest;
use crate::utils::password::hash_password;

#[tracing::instrument(
    name = "Adding a new user",
    // Don't show arguments
    skip(user_form, pool),
    fields(
        username = %user_form.username,
        email = %user_form
    )
)]
pub async fn register_user(
    user_form: web::Json<RegistrationRequest>,
    pool: web::Data<PgPool>
) -> HttpResponse {
    match insert_user(&user_form, &pool).await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::InternalServerError().finish()
    }
}

pub async fn insert_user(
    user_form: &web::Json<RegistrationRequest>,
    pool: &PgPool
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO users (id, username, password_hash, email, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        Uuid::new_v4(),
        &user_form.username,
        &hash_password(&user_form.password.expose_secret()),
        &user_form.email,
        Utc::now(),
        Utc::now()
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(())
}