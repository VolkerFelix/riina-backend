use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::models::common::ApiResponse;
use crate::middleware::auth::Claims;
use crate::utils::trailing_average;

#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerPoolEntry {
    pub user_id: Uuid,
    pub username: String,
    pub profile_picture_url: Option<String>,
    pub joined_pool_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
    pub stamina: f32,
    pub strength: f32,
    pub total_stats: f32,
    pub trailing_average: f32,
    pub rank: i32,
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
    // First, get all player pool entries with their stats
    let entries_result = sqlx::query!(
        r#"
        SELECT
            pp.user_id,
            u.username,
            u.profile_picture_url,
            pp.joined_pool_at,
            pp.last_active_at,
            COALESCE(ua.stamina, 0) as stamina,
            COALESCE(ua.strength, 0) as strength
        FROM player_pool pp
        INNER JOIN users u ON pp.user_id = u.id
        LEFT JOIN user_avatars ua ON pp.user_id = ua.user_id
        WHERE u.status = 'active'
        ORDER BY pp.joined_pool_at DESC
        "#
    )
    .fetch_all(pool.get_ref())
    .await;

    match entries_result {
        Ok(entries_raw) => {
            // Calculate trailing average and total stats for each entry
            let mut entries = Vec::new();

            for entry in entries_raw {
                let stamina = entry.stamina.unwrap_or(0.0);
                let strength = entry.strength.unwrap_or(0.0);
                let total_stats = stamina + strength;

                // Calculate trailing average
                let trailing_avg = trailing_average::calculate_trailing_average(
                    &pool,
                    entry.user_id
                ).await.unwrap_or(0.0);

                entries.push((entry, total_stats, trailing_avg));
            }

            // Sort by trailing average descending to calculate ranks
            entries.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

            // Assign ranks and create final entries
            let pool_entries: Vec<PlayerPoolEntry> = entries
                .iter()
                .enumerate()
                .map(|(index, (entry, total_stats, trailing_avg))| {
                    PlayerPoolEntry {
                        user_id: entry.user_id,
                        username: entry.username.clone(),
                        profile_picture_url: entry.profile_picture_url.clone(),
                        joined_pool_at: entry.joined_pool_at,
                        last_active_at: entry.last_active_at,
                        stamina: entry.stamina.unwrap_or(0.0),
                        strength: entry.strength.unwrap_or(0.0),
                        total_stats: *total_stats,
                        trailing_average: *trailing_avg,
                        rank: (index + 1) as i32,
                    }
                })
                .collect();

            let total_count = pool_entries.len();
            let response = PlayerPoolResponse {
                entries: pool_entries,
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
