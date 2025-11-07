use actix_web::{web, HttpResponse};
use secrecy::ExposeSecret;
use sqlx::PgPool;
use chrono::Utc;
use uuid::Uuid;
use std::sync::Arc;

use crate::models::user::{RegistrationRequest, UserRole, UserStatus};
use crate::utils::password::hash_password;
use crate::services::player_pool_events;

#[tracing::instrument(
    name = "Adding a new user",
    // Don't show arguments
    skip(user_form, pool, redis_client),
    fields(
        username = %user_form.username,
        email = %user_form
    )
)]
pub async fn register_user(
    user_form: web::Json<RegistrationRequest>,
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<redis::Client>>,
) -> HttpResponse {
    match insert_user(&user_form, &pool, &redis_client).await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::InternalServerError().finish()
    }
}

pub async fn insert_user(
    user_form: &web::Json<RegistrationRequest>,
    pool: &PgPool,
    redis_client: &Arc<redis::Client>,
) -> Result<(), sqlx::Error> {
    let user_id = Uuid::new_v4();
    let username = user_form.username.clone();

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

    // Add user to player pool (since they're active and not on any team)
    sqlx::query!(
        r#"
        INSERT INTO player_pool (user_id, last_active_at)
        VALUES ($1, $2)
        "#,
        user_id,
        Utc::now()
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to add user to player pool: {:?}", e);
        e
    })?;

    // Commit the transaction
    tx.commit().await?;

    tracing::info!("User {} successfully registered and added to player pool", user_id);

    // Publish player pool event (non-blocking, don't fail registration if it fails)
    if let Err(e) = player_pool_events::publish_player_joined(
        redis_client,
        &pool,
        user_id,
        username,
        None, // New users don't have a league yet
    ).await {
        tracing::warn!("Failed to publish player_joined event: {}", e);
    }

    Ok(())
}