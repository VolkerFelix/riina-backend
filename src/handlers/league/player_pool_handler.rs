use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::models::common::ApiResponse;
use crate::middleware::auth::Claims;

#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerPoolEntry {
    pub user_id: Uuid,
    pub username: String,
    pub profile_picture_url: Option<String>,
    pub joined_pool_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PlayerPoolResponse {
    pub entries: Vec<PlayerPoolEntry>,
    pub total_count: usize,
}

/// Get all players in the player pool
pub async fn get_player_pool(
    pool: web::Data<PgPool>,
    _claims: web::ReqData<Claims>,
) -> HttpResponse {
    let result = sqlx::query_as!(
        PlayerPoolEntry,
        r#"
        SELECT
            pp.user_id,
            u.username,
            u.profile_picture_url,
            pp.joined_pool_at,
            pp.last_active_at
        FROM player_pool pp
        INNER JOIN users u ON pp.user_id = u.id
        WHERE u.status = 'active'
        ORDER BY pp.joined_pool_at DESC
        "#
    )
    .fetch_all(pool.get_ref())
    .await;

    match result {
        Ok(entries) => {
            let total_count = entries.len();
            let response = PlayerPoolResponse {
                entries,
                total_count,
            };
            HttpResponse::Ok().json(ApiResponse::success(
                "Player pool retrieved successfully",
                response
            ))
        }
        Err(e) => {
            tracing::error!("Failed to fetch player pool: {}", e);
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to fetch player pool"
            ))
        }
    }
}
