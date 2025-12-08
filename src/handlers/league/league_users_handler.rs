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
    let trailing_averages = match trailing_average::calculate_trailing_averages_batch(pool, &user_ids).await {
        Ok(averages) => averages,
        Err(_) => std::collections::HashMap::new(),
    };

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
                stamina: row.stamina.unwrap_or(0.0) as f32,
                strength: row.strength.unwrap_or(0.0) as f32,
            },
            total_stats: (row.stamina.unwrap_or(0.0) + row.strength.unwrap_or(0.0)) as f32,
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

/// Fetch player pool users with their stats (for backwards compatibility - deprecated)
/// USE fetch_all_leaderboard_users instead to prevent duplicates
async fn fetch_player_pool_users(pool: &PgPool) -> Vec<LeagueUserWithStats> {
    let pool_result = sqlx::query!(
        r#"
        SELECT
            pp.user_id as "user_id!",
            u.username as "username!",
            u.email as "email!",
            u.profile_picture_url,
            COALESCE(ua.stamina, 0) as "stamina!",
            COALESCE(ua.strength, 0) as "strength!",
            COALESCE(ua.avatar_style, 'warrior') as "avatar_style!"
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
                        stamina: entry.stamina as f32,
                        strength: entry.strength as f32,
                    },
                    total_stats: (entry.stamina + entry.strength) as f32,
                    trailing_average: trailing_avg,
                    rank: 0,
                    avatar_style: entry.avatar_style,
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
    pub sort_by: Option<String>, // Accepted but currently ignored (always sorts by trailing_average)
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