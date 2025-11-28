use actix_web::{web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;
use serde_json::json;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::middleware::auth::Claims;
use crate::models::team::TeamRole;
use crate::models::common::PlayerStats;
use crate::utils::trailing_average;

/// Fetch player pool users with their stats
async fn fetch_player_pool_users(pool: &PgPool) -> Vec<LeagueUserWithStats> {
    let pool_result = sqlx::query!(
        r#"
        SELECT
            pp.user_id,
            u.username,
            u.email,
            u.profile_picture_url,
            COALESCE(ua.stamina, 0) as stamina,
            COALESCE(ua.strength, 0) as strength,
            COALESCE(ua.avatar_style, 'warrior') as avatar_style
        FROM player_pool pp
        INNER JOIN users u ON pp.user_id = u.id
        LEFT JOIN user_avatars ua ON pp.user_id = ua.user_id
        WHERE u.status = 'active'
        "#
    )
    .fetch_all(pool)
    .await;

    match pool_result {
        Ok(entries) => {
            let user_ids: Vec<Uuid> = entries.iter().map(|e| e.user_id).collect();

            let trailing_averages = match trailing_average::calculate_trailing_averages_batch(pool, &user_ids).await {
                Ok(averages) => averages,
                Err(_) => std::collections::HashMap::new(),
            };

            entries.into_iter().map(|entry| {
                let trailing_avg = trailing_averages.get(&entry.user_id).copied().unwrap_or(0.0);
                LeagueUserWithStats {
                    user_id: entry.user_id,
                    username: entry.username,
                    email: entry.email,
                    team_id: None,
                    team_name: None,
                    team_role: TeamRole::Member,
                    team_status: None,
                    joined_at: None,
                    stats: PlayerStats {
                        stamina: entry.stamina.unwrap_or(0.0) as f32,
                        strength: entry.strength.unwrap_or(0.0) as f32,
                    },
                    total_stats: (entry.stamina.unwrap_or(0.0) + entry.strength.unwrap_or(0.0)) as f32,
                    trailing_average: trailing_avg,
                    rank: 0,
                    avatar_style: entry.avatar_style.unwrap_or_else(|| "warrior".to_string()),
                    is_online: false,
                    profile_picture_url: entry.profile_picture_url,
                }
            }).collect()
        }
        Err(_) => Vec::new(),
    }
}

/// Enhanced team member with user stats
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LeagueUserWithStats {
    pub user_id: Uuid,
    pub username: String,
    pub email: String,
    pub team_id: Option<Uuid>,
    pub team_name: Option<String>,
    pub team_role: TeamRole,
    pub team_status: Option<String>,
    pub joined_at: Option<DateTime<Utc>>,
    pub stats: PlayerStats,
    pub total_stats: f32,
    pub trailing_average: f32,
    pub rank: i32,
    pub avatar_style: String,
    pub is_online: bool,
    pub profile_picture_url: Option<String>,
}

// Using PlayerStats from common module instead of duplicate PlayerStats

/// Response for league users with stats
#[derive(Debug, Serialize, Deserialize)]
pub struct LeagueUsersResponse {
    pub success: bool,
    pub data: Vec<LeagueUserWithStats>,
    pub total_count: usize,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,
}

/// Query parameters for pagination
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub page: Option<usize>,
    pub page_size: Option<usize>,
    pub sort_by: Option<String>,
}

/// Get all users in the same league with their stats
/// This endpoint returns all users who are members of teams in active leagues
#[tracing::instrument(
    name = "Get league users with stats",
    skip(pool, claims),
    fields(
        username = %claims.username
    )
)]
pub async fn get_league_users_with_stats(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse> {
    let requester_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Invalid user ID in claims: {}", e);
            return Ok(HttpResponse::BadRequest().json(json!({
                "success": false,
                "message": "Invalid user ID"
            })));
        }
    };

    tracing::info!("Fetching league users with stats for requester: {}", requester_id);

    // First, check if the requester is part of a team
    let _requester_team = match sqlx::query!(
        r#"
        SELECT team_id
        FROM team_members
        WHERE user_id = $1
        "#,
        requester_id
    )
    .fetch_optional(pool.get_ref())
    .await
    {
        Ok(result) => result.map(|row| row.team_id),
        Err(e) => {
            tracing::error!("Failed to check requester team membership: {}", e);
            return Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to verify team membership"
            })));
        }
    };

    // Set pagination defaults
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).min(200).max(1); // Default 20, max 200
    let offset = (page - 1) * page_size;
    let sort_by = query.sort_by.as_deref().unwrap_or("total_stats");

    // First, get the total count of league users
    let total_count = match sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM users u
        INNER JOIN team_members tm ON u.id = tm.user_id
        INNER JOIN teams t ON tm.team_id = t.id
        "#
    )
    .fetch_one(pool.get_ref())
    .await
    {
        Ok(result) => result.count.unwrap_or(0) as usize,
        Err(e) => {
            tracing::error!("Failed to count league users: {}", e);
            return Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to count league users"
            })));
        }
    };

    // Get all team users with their basic stats (always sort by trailing_average)
    let league_users: Vec<LeagueUserWithStats> = match sqlx::query!(
        r#"
        SELECT
            u.id as user_id,
            u.username,
            u.email,
            u.profile_picture_url,
            tm.team_id as team_id,
            t.team_name as team_name,
            tm.role as team_role,
            tm.status as team_status,
            tm.joined_at as joined_at,
            COALESCE(ua.stamina, 0.0) as stamina,
            COALESCE(ua.strength, 0.0) as strength,
            COALESCE(ua.stamina + ua.strength, 0.0) as total_stats,
            COALESCE(ua.avatar_style, 'warrior') as avatar_style,
            false as is_online
        FROM users u
        INNER JOIN team_members tm ON u.id = tm.user_id
        INNER JOIN teams t ON tm.team_id = t.id
        LEFT JOIN user_avatars ua ON u.id = ua.user_id
        ORDER BY
            t.team_name ASC,
            CASE tm.role
                WHEN 'owner' THEN 1
                WHEN 'admin' THEN 2
                WHEN 'member' THEN 3
            END,
            tm.joined_at ASC
        LIMIT $1 OFFSET $2
        "#,
        page_size as i64,
        offset as i64
    )
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(users) => {
            // Extract user IDs for batch trailing average calculation
            let user_ids: Vec<Uuid> = users.iter().map(|row| row.user_id).collect();

            // Calculate trailing averages for all users in batch
            let trailing_averages = match trailing_average::calculate_trailing_averages_batch(
                pool.get_ref(),
                &user_ids
            ).await {
                Ok(averages) => averages,
                Err(e) => {
                    tracing::error!("Failed to calculate trailing averages: {}", e);
                    std::collections::HashMap::new()
                }
            };

            // Create a list of users with their trailing averages for sorting
            let mut users_with_trailing: Vec<(LeagueUserWithStats, f32)> = users.into_iter().map(|row| {
                let trailing_avg = trailing_averages.get(&row.user_id).copied().unwrap_or(0.0);

                let user_stats = LeagueUserWithStats {
                    user_id: row.user_id,
                    username: row.username,
                    email: row.email,
                    team_id: Some(row.team_id),
                    team_name: Some(row.team_name),
                    team_role: match row.team_role.as_str() {
                        "owner" => TeamRole::Owner,
                        "admin" => TeamRole::Admin,
                        "member" => TeamRole::Member,
                        _ => TeamRole::Member,
                    },
                    team_status: Some(row.team_status),
                    joined_at: Some(row.joined_at),
                    stats: PlayerStats {
                        stamina: row.stamina.unwrap_or(50.0),
                        strength: row.strength.unwrap_or(50.0),
                    },
                    total_stats: row.total_stats.unwrap_or(100.0),
                    trailing_average: trailing_avg,
                    rank: 0, // Will be set after sorting
                    avatar_style: row.avatar_style.unwrap_or_else(|| "warrior".to_string()),
                    is_online: row.is_online.unwrap_or(false),
                    profile_picture_url: row.profile_picture_url,
                };

                (user_stats, trailing_avg)
            }).collect();

            // Sort by trailing average (descending), then by user_id for stable ordering
            users_with_trailing.sort_by(|a, b| {
                match b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal) {
                    std::cmp::Ordering::Equal => a.0.user_id.cmp(&b.0.user_id),
                    other => other,
                }
            });

            // Assign ranks and extract final results
            let mut team_users: Vec<LeagueUserWithStats> = users_with_trailing.into_iter().enumerate().map(|(index, (mut user_stats, _))| {
                user_stats.rank = (index + 1) as i32;
                user_stats
            }).collect();

            // Fetch player pool users and add them
            let pool_users = fetch_player_pool_users(pool.get_ref()).await;
            team_users.extend(pool_users);

            // Re-sort all users (team + pool) by trailing average
            team_users.sort_by(|a, b| {
                match b.trailing_average.partial_cmp(&a.trailing_average).unwrap_or(std::cmp::Ordering::Equal) {
                    std::cmp::Ordering::Equal => a.user_id.cmp(&b.user_id),
                    other => other,
                }
            });

            // Re-assign ranks after merging
            team_users.into_iter().enumerate().map(|(index, mut user_stats)| {
                user_stats.rank = (index + 1) as i32;
                user_stats
            }).collect()
        }
        Err(e) => {
            tracing::error!("Failed to fetch league users with stats: {}", e);
            return Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to fetch league users"
            })));
        }
    };

    let total_pages = total_count.div_ceil(page_size);

    tracing::info!(
        "Successfully fetched {} league users with stats (page {} of {}, {} per page)",
        league_users.len(),
        page,
        total_pages,
        page_size
    );

    Ok(HttpResponse::Ok().json(LeagueUsersResponse {
        success: true,
        data: league_users,
        total_count,
        page,
        page_size,
        total_pages,
    }))
}