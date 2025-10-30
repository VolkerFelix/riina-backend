use actix_web::{web, HttpResponse};
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;

use crate::middleware::auth::Claims;
use crate::models::profile::{UserProfileResponse, GameStats};
use crate::models::common::ApiResponse;
use crate::utils::trailing_average;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UserProfileQuery {
    pub user_id: Option<String>,
}

#[tracing::instrument(
    name = "Get user profile",
    skip(pool, claims, query),
    fields(username = %claims.username)
)]
pub async fn get_user_profile(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    query: web::Query<UserProfileQuery>
) -> HttpResponse {
    // Check if a user_id query parameter was provided
    let user_id = if let Some(user_id_str) = &query.user_id {
        // Requesting another user's profile
        match Uuid::parse_str(user_id_str) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("Failed to parse user_id query parameter: {}", e);
                return HttpResponse::BadRequest().json(
                    ApiResponse::<()>::error("Invalid user_id parameter")
                );
            }
        }
    } else {
        // Default: get the current user's own profile
        match Uuid::parse_str(&claims.sub) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("Failed to parse user ID: {}", e);
                return HttpResponse::BadRequest().json(
                    ApiResponse::<()>::error("Invalid user ID")
                );
            }
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

    // Count MVP badges
    let mvp_count = match sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM game_summaries
        WHERE mvp_user_id = $1
        "#,
        user_id
    )
    .fetch_one(&**pool)
    .await
    {
        Ok(row) => row.count.unwrap_or(0),
        Err(e) => {
            tracing::warn!("Failed to count MVP badges for user {}: {}", user_id, e);
            0
        }
    };

    // Count LVP badges
    let lvp_count = match sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM game_summaries
        WHERE lvp_user_id = $1
        "#,
        user_id
    )
    .fetch_one(&**pool)
    .await
    {
        Ok(row) => row.count.unwrap_or(0),
        Err(e) => {
            tracing::warn!("Failed to count LVP badges for user {}: {}", user_id, e);
            0
        }
    };

    // Calculate average exercise minutes per day (last 30 days)
    let avg_exercise_minutes = match sqlx::query!(
        r#"
        SELECT COALESCE(AVG(duration_minutes)::FLOAT, 0.0) as avg_minutes
        FROM workout_data
        WHERE user_id = $1
        AND workout_start >= NOW() - INTERVAL '30 days'
        AND duration_minutes IS NOT NULL
        "#,
        user_id
    )
    .fetch_one(&**pool)
    .await
    {
        Ok(row) => row.avg_minutes.unwrap_or(0.0) as f32,
        Err(e) => {
            tracing::warn!("Failed to calculate average exercise minutes for user {}: {}", user_id, e);
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
        mvp_count,
        lvp_count,
        avg_exercise_minutes_per_day: avg_exercise_minutes,
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
    // Get all active users with their trailing averages
    let active_users = sqlx::query!(
        r#"
        SELECT u.id as user_id
        FROM users u
        INNER JOIN team_members tm ON u.id = tm.user_id AND tm.status = 'active'
        "#
    )
    .fetch_all(pool)
    .await?;

    let user_ids: Vec<Uuid> = active_users.iter().map(|row| row.user_id).collect();

    // Calculate trailing averages for all users in batch
    let trailing_averages = trailing_average::calculate_trailing_averages_batch(pool, &user_ids).await?;

    // Sort users by trailing average (descending)
    let mut users_with_avg: Vec<(Uuid, f32)> = user_ids.iter()
        .map(|&id| (id, trailing_averages.get(&id).copied().unwrap_or(0.0)))
        .collect();

    users_with_avg.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Find the rank of the target user
    let rank = users_with_avg.iter()
        .position(|(id, _)| *id == user_id)
        .map(|pos| (pos + 1) as i32)
        .unwrap_or(999);

    Ok(rank)
}