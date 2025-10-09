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

/// Enhanced team member with user stats
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LeagueUserWithStats {
    pub user_id: Uuid,
    pub username: String,
    pub email: String,
    pub team_id: Uuid,
    pub team_name: String,
    pub team_role: TeamRole,
    pub team_status: String,
    pub joined_at: DateTime<Utc>,
    pub stats: PlayerStats,
    pub total_stats: f32,
    pub trailing_average: f32,
    pub rank: i32,
    pub avatar_style: String,
    pub is_online: bool,
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
        WHERE user_id = $1 AND status = 'active'
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
    let page_size = query.page_size.unwrap_or(20).min(100).max(1); // Default 20, max 100
    let offset = (page - 1) * page_size;
    let sort_by = query.sort_by.as_deref().unwrap_or("total_stats");

    // First, get the total count of league users
    let total_count = match sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM users u
        INNER JOIN team_members tm ON u.id = tm.user_id AND tm.status = 'active'
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

    // For now, we'll return all users who are in teams (league participants)
    // In the future, this could be filtered by specific leagues or seasons
    let league_users: Vec<LeagueUserWithStats> = if sort_by == "trailing_average" {
        // First, get all users with their basic stats
        match sqlx::query!(
            r#"
            SELECT 
                u.id as user_id,
                u.username,
                u.email,
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
            INNER JOIN team_members tm ON u.id = tm.user_id AND tm.status = 'active'
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
                        team_id: row.team_id,
                        team_name: row.team_name,
                        team_role: match row.team_role.as_str() {
                            "owner" => TeamRole::Owner,
                            "admin" => TeamRole::Admin,
                            "member" => TeamRole::Member,
                            _ => TeamRole::Member,
                        },
                        team_status: row.team_status,
                        joined_at: row.joined_at,
                        stats: PlayerStats {
                            stamina: row.stamina.unwrap_or(50.0),
                            strength: row.strength.unwrap_or(50.0),
                        },
                        total_stats: row.total_stats.unwrap_or(100.0),
                        trailing_average: trailing_avg,
                        rank: 0, // Will be set after sorting
                        avatar_style: row.avatar_style.unwrap_or_else(|| "warrior".to_string()),
                        is_online: row.is_online.unwrap_or(false),
                    };
                    
                    (user_stats, trailing_avg)
                }).collect();
                
                // Sort by trailing average (descending)
                users_with_trailing.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                
                // Assign ranks and extract final results
                users_with_trailing.into_iter().enumerate().map(|(index, (mut user_stats, _))| {
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
        }
    } else {
        // Original query for total_stats sorting
        match sqlx::query!(
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
            SELECT 
                u.id as user_id,
                u.username,
                u.email,
                tm.team_id as team_id,
                t.team_name as team_name,
                tm.role as team_role,
                tm.status as team_status,
                tm.joined_at as joined_at,
                COALESCE(ua.stamina, 0.0) as stamina,
                COALESCE(ua.strength, 0.0) as strength,
                COALESCE(ua.stamina + ua.strength, 0.0) as total_stats,
                COALESCE(ur.rank, 999) as rank,
                COALESCE(ua.avatar_style, 'warrior') as avatar_style,
                false as is_online -- TODO: Implement real online status from websocket connections
            FROM users u
            INNER JOIN team_members tm ON u.id = tm.user_id AND tm.status = 'active'
            INNER JOIN teams t ON tm.team_id = t.id
            LEFT JOIN user_avatars ua ON u.id = ua.user_id
            LEFT JOIN user_rankings ur ON u.id = ur.user_id
            ORDER BY 
                COALESCE(ur.rank, 999) ASC,
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
                
                users.into_iter().map(|row| {
                    LeagueUserWithStats {
                        user_id: row.user_id,
                        username: row.username,
                        email: row.email,
                        team_id: row.team_id,
                        team_name: row.team_name,
                        team_role: match row.team_role.as_str() {
                            "owner" => TeamRole::Owner,
                            "admin" => TeamRole::Admin,
                            "member" => TeamRole::Member,
                            _ => TeamRole::Member, // Default fallback
                        },
                        team_status: row.team_status,
                        joined_at: row.joined_at,
                        stats: PlayerStats {
                            stamina: row.stamina.unwrap_or(50.0),
                            strength: row.strength.unwrap_or(50.0),
                        },
                        total_stats: row.total_stats.unwrap_or(100.0),
                        trailing_average: trailing_averages.get(&row.user_id).copied().unwrap_or(0.0),
                        rank: row.rank.unwrap_or(999) as i32,
                        avatar_style: row.avatar_style.unwrap_or_else(|| "warrior".to_string()),
                        is_online: row.is_online.unwrap_or(false),
                    }
                }).collect()
            }
            Err(e) => {
                tracing::error!("Failed to fetch league users with stats: {}", e);
                return Ok(HttpResponse::InternalServerError().json(json!({
                    "success": false,
                    "message": "Failed to fetch league users"
                })));
            }
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