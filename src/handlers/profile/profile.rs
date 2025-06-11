use actix_web::{web, HttpResponse};
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;

use crate::middleware::auth::Claims;
use crate::models::profile::{UserProfileResponse, GameStats};

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
            return HttpResponse::BadRequest().json(json!({
                "error": "Invalid user ID"
            }));
        }
    };

    tracing::info!("Fetching user profile for: {}", user_id);

    // Get user basic info
    let user_info = match sqlx::query!(
        r#"
        SELECT id, username, created_at
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
            return HttpResponse::InternalServerError().json(json!({
                "error": "Failed to fetch user profile"
            }));
        }
    };

    tracing::info!("Fetching avatar stats for: {}", user_id);

    // Get user game stats (avatar stats)
    let game_stats = match sqlx::query!(
        r#"
        SELECT stamina, strength, experience_points, avatar_level, avatar_style
        FROM user_avatars 
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(&**pool)
    .await
    {
        Ok(Some(avatar)) => GameStats {
            stamina: avatar.stamina,
            strength: avatar.strength,
            experience_points: avatar.experience_points,
        },
        Ok(None) => {
            // Create default avatar if none exists
            match create_default_avatar(&pool, user_id).await {
                Ok(stats) => stats,
                Err(_) => GameStats {
                    stamina: 50,
                    strength: 50,
                    experience_points: 0,
                }
            }
        }
        Err(e) => {
            tracing::error!("Database error fetching avatar: {}", e);
            GameStats {
                stamina: 50,
                strength: 50,
                experience_points: 0,
            }
        }
    };

    tracing::info!("Getting stats for user: {}", user_id);
    // Get user rank from leaderboard
    let rank = get_user_rank(&pool, user_id).await.unwrap_or(999);

    // Get avatar style and level
    let (level, avatar_style) = match sqlx::query!(
        r#"
        SELECT avatar_level, avatar_style
        FROM user_avatars 
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(&**pool)
    .await
    {
        Ok(Some(avatar)) => (avatar.avatar_level, avatar.avatar_style.unwrap_or_else(|| "warrior".to_string())),
        Ok(None) => (1, "warrior".to_string()),
        Err(_) => (1, "warrior".to_string()),
    };

    let total_stats = game_stats.stamina + game_stats.strength;

    let profile = UserProfileResponse {
        id: user_info.id,
        username: user_info.username,
        level,
        experience_points: game_stats.experience_points,
        stats: game_stats,
        rank,
        avatar_style,
        total_stats,
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
        INSERT INTO user_avatars (user_id, stamina, strength, experience_points, avatar_level, avatar_style)
        VALUES ($1, 50, 50, 0, 1, 'warrior')
        ON CONFLICT (user_id) DO NOTHING
        "#,
        user_id
    )
    .execute(pool)
    .await?;

    Ok(GameStats {
        stamina: 50,
        strength: 50,
        experience_points: 0,
    })
}

async fn get_user_rank(pool: &PgPool, user_id: Uuid) -> Result<i32, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        WITH ranked_users AS (
            SELECT 
                user_id,
                ROW_NUMBER() OVER (ORDER BY (stamina + strength) DESC, experience_points DESC) as rank
            FROM user_avatars
        )
        SELECT rank::int as rank
        FROM ranked_users
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    match result {
        Some(row) => Ok(row.rank.unwrap_or(999)),
        None => Ok(999), // User not found in rankings
    }
}