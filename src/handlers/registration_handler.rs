use actix_web::{web, HttpResponse};
use secrecy::ExposeSecret;
use sqlx::PgPool;
use chrono::Utc;
use uuid::Uuid;

use crate::models::user::{RegistrationRequest, UserRole, UserStatus};
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
    let user_id = Uuid::new_v4();
    
    // Start a transaction to ensure both user and avatar are created atomically
    let mut tx = pool.begin().await?;
    
    // Insert user
    sqlx::query!(
        r#"
        INSERT INTO users (id, username, password_hash, email, role, status, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
        user_id,
        &user_form.username,
        &hash_password(&user_form.password.expose_secret()),
        &user_form.email,
        UserRole::User.to_string(),
        UserStatus::Active.to_string(),
        Utc::now(),
        Utc::now()
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute user insert query: {:?}", e);
        e
    })?;
    
    // Create default avatar for the user
    sqlx::query!(
        r#"
        INSERT INTO user_avatars (user_id, stamina, strength, avatar_style)
        VALUES ($1, 0, 0, 'warrior')
        "#,
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute avatar insert query: {:?}", e);
        e
    })?;
    
    // Commit the transaction
    tx.commit().await?;
    Ok(())
}