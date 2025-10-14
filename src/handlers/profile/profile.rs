use actix_web::{web, HttpResponse};
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;

use crate::middleware::auth::Claims;
use crate::models::profile::{UserProfileResponse, GameStats};
use crate::models::common::ApiResponse;
use crate::utils::trailing_average;

#[tracing::instrument(
    name = "Get user profile",
    skip(pool, claims),
    fields(username = %claims.username)
)]
pub async fn get_user_profile(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::BadRequest().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };

    tracing::info!("Fetching user profile for: {}", user_id);

    // Get user basic info
    let user_info = match sqlx::query!(
        r#"
        SELECT id, username, created_at, profile_picture_url
        FROM users 
        WHERE id = $1
        "#,
        user_id
    )
    .fetch_optional(&**pool)
    .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return HttpResponse::NotFound().json(json!({
                "error": "User not found"
            }));
        }
        Err(e) => {
            tracing::error!("Database error fetching user: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to fetch user profile")
            );
        }
    };

    tracing::info!("Fetching avatar stats for: {}", user_id);

    // Get user game stats (avatar stats)
    let game_stats = match sqlx::query!(
        r#"
        SELECT stamina, strength, avatar_style
        FROM user_avatars 
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(&**pool)
    .await
    {
        Ok(Some(avatar)) => GameStats {
            stamina: avatar.stamina as f32,
            strength: avatar.strength as f32,
        },
        Ok(None) => {
            // Create default avatar if none exists
            match create_default_avatar(&pool, user_id).await {
                Ok(stats) => stats,
                Err(_) => GameStats {
                    stamina: 50.0,
                    strength: 50.0,
                }
            }
        }
        Err(e) => {
            tracing::error!("Database error fetching avatar: {}", e);
            GameStats {
                stamina: 50.0,
                strength: 50.0,
            }
        }
    };

    tracing::info!("Getting stats for user: {}", user_id);
    // Get user rank from leaderboard
    let rank = get_user_rank(&pool, user_id).await.unwrap_or(999);

    // Get avatar style
    let avatar_style = match sqlx::query!(
        r#"
        SELECT avatar_style
        FROM user_avatars 
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(&**pool)
    .await
    {
        Ok(Some(avatar)) => avatar.avatar_style.unwrap_or_else(|| "warrior".to_string()),
        Ok(None) => "warrior".to_string(),
        Err(_) => "warrior".to_string(),
    };

    let total_stats = game_stats.stamina + game_stats.strength;

    // Calculate trailing average
    let trailing_avg = match trailing_average::calculate_trailing_average(&pool, user_id).await {
        Ok(avg) => avg,
        Err(e) => {
            tracing::warn!("Failed to calculate trailing average for user {}: {}", user_id, e);
            0.0
        }
    };

    let profile = UserProfileResponse {
        id: user_info.id,
        username: user_info.username,
        stats: game_stats,
        rank,
        avatar_style,
        total_stats,
        trailing_average: trailing_avg,
        profile_picture_url: user_info.profile_picture_url,
        created_at: user_info.created_at,
        last_login: None,
    };

    tracing::info!("Successfully retrieved profile for user: {}", claims.username);
    HttpResponse::Ok().json(json!({
        "success": true,
        "data": profile
    }))
}

async fn create_default_avatar(pool: &PgPool, user_id: Uuid) -> Result<GameStats, sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO user_avatars (user_id, stamina, strength, avatar_style)
        VALUES ($1, 50, 50, 'warrior')
        ON CONFLICT (user_id) DO NOTHING
        "#,
        user_id
    )
    .execute(pool)
    .await?;

    Ok(GameStats {
        stamina: 50.0,
        strength: 50.0,
    })
}

async fn get_user_rank(pool: &PgPool, user_id: Uuid) -> Result<i32, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        WITH user_rankings AS (
            SELECT 
                u.id as user_id,
                COALESCE(ua.stamina + ua.strength, 0.0) as total_stats,
                ROW_NUMBER() OVER (ORDER BY COALESCE(ua.stamina + ua.strength, 0.0) DESC) as rank
            FROM users u
            INNER JOIN team_members tm ON u.id = tm.user_id AND tm.status = 'active'
            LEFT JOIN user_avatars ua ON u.id = ua.user_id
        )
        SELECT rank::int as rank
        FROM user_rankings
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    match result {
        Some(row) => Ok(row.rank.unwrap_or(999)),
        None => Ok(999), // User not found in league rankings
    }
}