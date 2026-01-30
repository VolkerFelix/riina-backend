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

/// Fetch ALL eligible users for the leaderboard (team members + free agents)
/// This function ensures mutual exclusivity: users in teams are NOT included as free agents
pub async fn fetch_all_leaderboard_users(pool: &PgPool) -> Result<Vec<LeagueUserWithStats>, sqlx::Error> {
    // Use a UNION query to get both team members and free agents
    // Free agents are excluded if they have ANY team membership (to prevent duplicates)
    let all_users = sqlx::query!(
        r#"
        -- Team members
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
            COALESCE(ua.avatar_style, 'warrior') as avatar_style
        FROM users u
        INNER JOIN team_members tm ON u.id = tm.user_id
        INNER JOIN teams t ON tm.team_id = t.id
        LEFT JOIN user_avatars ua ON u.id = ua.user_id

        UNION

        -- Free agents (only those NOT in any team)
        SELECT
            u.id as user_id,
            u.username,
            u.email,
            u.profile_picture_url,
            NULL as team_id,
            NULL as team_name,
            'member' as team_role,
            NULL as team_status,
            NULL as joined_at,
            COALESCE(ua.stamina, 0.0) as stamina,
            COALESCE(ua.strength, 0.0) as strength,
            COALESCE(ua.avatar_style, 'warrior') as avatar_style
        FROM player_pool pp
        INNER JOIN users u ON pp.user_id = u.id
        LEFT JOIN user_avatars ua ON pp.user_id = ua.user_id
        WHERE u.status = 'active'
        AND NOT EXISTS (
            SELECT 1 FROM team_members tm WHERE tm.user_id = u.id
        )
        "#
    )
    .fetch_all(pool)
    .await?;

    // Extract user IDs for batch trailing average calculation
    let user_ids: Vec<Uuid> = all_users.iter().filter_map(|row| row.user_id).collect();

    // Calculate trailing averages for all users in batch
    let trailing_averages = trailing_average::calculate_trailing_averages_batch(pool, &user_ids)
        .await
        .unwrap_or_default();

    // Transform to LeagueUserWithStats
    // UNION makes all fields nullable, so we need to handle Option types
    let mut users_with_stats: Vec<LeagueUserWithStats> = all_users.into_iter().filter_map(|row| {
        // Skip rows with null user_id (shouldn't happen but UNION can make fields nullable)
        let user_id = row.user_id?;
        let username = row.username?;
        let email = row.email?;

        let trailing_avg = trailing_averages.get(&user_id).copied().unwrap_or(0.0);

        Some(LeagueUserWithStats {
            user_id,
            username,
            email,
            team_id: row.team_id,
            team_name: row.team_name,
            team_role: match row.team_role.as_deref() {
                Some("owner") => TeamRole::Owner,
                Some("admin") => TeamRole::Admin,
                _ => TeamRole::Member,
            },
            team_status: row.team_status,
            joined_at: row.joined_at,
            stats: PlayerStats {
                stamina: row.stamina.unwrap_or(0.0),
                strength: row.strength.unwrap_or(0.0),
            },
            total_stats: (row.stamina.unwrap_or(0.0) + row.strength.unwrap_or(0.0)),
            trailing_average: trailing_avg,
            rank: 0, // Will be assigned after sorting
            avatar_style: row.avatar_style.unwrap_or_else(|| "warrior".to_string()),
            is_online: false,
            profile_picture_url: row.profile_picture_url,
        })
    }).collect();

    // Sort by trailing average (descending), then by user_id for stable ordering
    users_with_stats.sort_by(|a, b| {
        match b.trailing_average.partial_cmp(&a.trailing_average).unwrap_or(std::cmp::Ordering::Equal) {
            std::cmp::Ordering::Equal => a.user_id.cmp(&b.user_id),
            other => other,
        }
    });

    // Assign ranks
    for (index, user) in users_with_stats.iter_mut().enumerate() {
        user.rank = (index + 1) as i32;
    }

    Ok(users_with_stats)
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
    let Some(requester_id) = claims.user_id() else {
        tracing::error!("Invalid user ID in claims");
        return Ok(HttpResponse::BadRequest().json(json!({
            "success": false,
            "message": "Invalid user ID"
        })));
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
    let page_size = query.page_size.unwrap_or(20).clamp(1, 200); // Default 20, max 200

    // Fetch ALL leaderboard users (team members + free agents, ensuring no duplicates)
    let all_users = match fetch_all_leaderboard_users(pool.get_ref()).await {
        Ok(users) => users,
        Err(e) => {
            tracing::error!("Failed to fetch league users with stats: {}", e);
            return Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to fetch league users"
            })));
        }
    };

    let total_count = all_users.len();

    // Apply pagination
    let offset = (page - 1) * page_size;
    let league_users: Vec<LeagueUserWithStats> = all_users
        .into_iter()
        .skip(offset)
        .take(page_size)
        .collect();

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

/// Simple user info for mentions/tagging
#[derive(Debug, Serialize, Deserialize)]
pub struct UserSearchResult {
    pub user_id: Uuid,
    pub username: String,
    pub profile_picture_url: Option<String>,
}

/// Database row struct for user search queries
#[derive(Debug)]
struct UserSearchRow {
    user_id: Uuid,
    username: String,
    profile_picture_url: Option<String>,
}

/// Response for user search
#[derive(Debug, Serialize, Deserialize)]
pub struct UserSearchResponse {
    pub success: bool,
    pub data: Vec<UserSearchResult>,
}

/// Query parameters for user search
#[derive(Debug, Deserialize)]
pub struct UserSearchParams {
    pub q: Option<String>,
    pub limit: Option<i64>,
}

/// Search users by username (for mentions/tagging)
/// Returns lightweight user info without stats
#[tracing::instrument(
    name = "Search users",
    skip(pool, claims),
    fields(
        username = %claims.username
    )
)]
pub async fn search_users(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    query: web::Query<UserSearchParams>,
) -> Result<HttpResponse> {
    let Some(_requester_id) = claims.user_id() else {
        tracing::error!("Invalid user ID in claims");
        return Ok(HttpResponse::BadRequest().json(json!({
            "success": false,
            "message": "Invalid user ID"
        })));
    };

    let search_query = query.q.clone().unwrap_or_default().to_lowercase();
    let limit = query.limit.unwrap_or(20).min(50); // Max 50 results

    tracing::info!("Searching users with query: '{}', limit: {}", search_query, limit);

    let users = if search_query.is_empty() {
        // Return recent active users if no query
        sqlx::query_as!(
            UserSearchRow,
            r#"
            SELECT
                u.id as user_id,
                u.username,
                u.profile_picture_url
            FROM users u
            WHERE u.status = 'active'
            ORDER BY u.created_at DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(pool.get_ref())
        .await
    } else {
        // Search by username
        sqlx::query_as!(
            UserSearchRow,
            r#"
            SELECT
                u.id as user_id,
                u.username,
                u.profile_picture_url
            FROM users u
            WHERE u.status = 'active'
            AND LOWER(u.username) LIKE $1
            ORDER BY
                CASE WHEN LOWER(u.username) = $2 THEN 0 ELSE 1 END,
                u.username
            LIMIT $3
            "#,
            format!("%{}%", search_query),
            search_query,
            limit
        )
        .fetch_all(pool.get_ref())
        .await
    };

    match users {
        Ok(rows) => {
            let results: Vec<UserSearchResult> = rows
                .into_iter()
                .map(|row| UserSearchResult {
                    user_id: row.user_id,
                    username: row.username,
                    profile_picture_url: row.profile_picture_url,
                })
                .collect();

            tracing::info!("Found {} users matching query", results.len());

            Ok(HttpResponse::Ok().json(UserSearchResponse {
                success: true,
                data: results,
            }))
        }
        Err(e) => {
            tracing::error!("Failed to search users: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to search users"
            })))
        }
    }
}