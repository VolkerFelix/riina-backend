use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;
use chrono::{DateTime, Utc};

use crate::middleware::auth::Claims;

#[derive(Debug, Serialize)]
pub struct FeedWorkoutItem {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub profile_picture_url: Option<String>,
    pub workout_date: DateTime<Utc>,
    pub workout_start: DateTime<Utc>,
    pub workout_end: DateTime<Utc>,
    pub duration_minutes: Option<i32>,
    pub calories_burned: Option<i32>,
    pub activity_name: Option<String>,
    pub avg_heart_rate: Option<i32>,
    pub max_heart_rate: Option<i32>,
    pub heart_rate_zones: Option<serde_json::Value>,
    pub stamina_gained: f32,
    pub strength_gained: f32,
    pub total_points_gained: f32,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub reaction_count: i64,
    pub comment_count: i64,
    pub user_has_reacted: bool,
    pub visibility: String,
    pub is_participating_in_live_game: bool,
    pub live_game_info: Option<LiveGameInfo>,
}

#[derive(Debug, Serialize, Clone)]
pub struct LiveGameInfo {
    pub game_id: Uuid,
    pub opponent_team_name: String,
    pub user_team_name: String,
}

#[derive(Debug, Deserialize)]
pub struct NewsfeedQuery {
    pub limit: Option<i32>,
    pub cursor: Option<String>, // ISO 8601 timestamp for cursor-based pagination
}

/// Get live game information for a user if they are participating
async fn get_user_live_game_info(user_id: Uuid, pool: &PgPool) -> Option<LiveGameInfo> {
    let result = sqlx::query!(
        r#"
        SELECT 
            g.id as game_id,
            CASE 
                WHEN tm.team_id = g.home_team_id THEN ht.team_name
                WHEN tm.team_id = g.away_team_id THEN at.team_name
                ELSE NULL
            END as user_team_name,
            CASE 
                WHEN tm.team_id = g.home_team_id THEN at.team_name
                WHEN tm.team_id = g.away_team_id THEN ht.team_name
                ELSE NULL
            END as opponent_team_name
        FROM games g
        JOIN team_members tm ON (tm.team_id = g.home_team_id OR tm.team_id = g.away_team_id)
        JOIN teams ht ON ht.id = g.home_team_id
        JOIN teams at ON at.id = g.away_team_id
        WHERE g.status = 'in_progress' 
        AND tm.user_id = $1 
        AND tm.status = 'active'
        LIMIT 1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await;

    match result {
        Ok(Some(row)) => {
            if let (Some(user_team), Some(opponent_team)) = (row.user_team_name, row.opponent_team_name) {
                Some(LiveGameInfo {
                    game_id: row.game_id,
                    user_team_name: user_team,
                    opponent_team_name: opponent_team,
                })
            } else {
                None
            }
        },
        Ok(None) => None,
        Err(e) => {
            tracing::error!("Failed to get user live game info: {}", e);
            None
        }
    }
}

#[tracing::instrument(
    name = "Get newsfeed",
    skip(pool, claims),
    fields(username = %claims.username)
)]
pub async fn get_newsfeed(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    query: web::Query<NewsfeedQuery>,
) -> HttpResponse {
    let current_user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "error": "Invalid user ID"
            }));
        }
    };

    let limit = query.limit.unwrap_or(20).min(50); // Max 50 items per request

    // Parse cursor if provided
    let cursor_datetime = match &query.cursor {
        Some(cursor) => {
            match DateTime::parse_from_rfc3339(cursor) {
                Ok(dt) => Some(dt.with_timezone(&Utc)),
                Err(e) => {
                    tracing::error!("Failed to parse cursor: {}", e);
                    return HttpResponse::BadRequest().json(json!({
                        "error": "Invalid cursor format"
                    }));
                }
            }
        },
        None => None
    };

    // Fetch public workouts with user info and social counts
    let workouts: Vec<FeedWorkoutItem> = match sqlx::query!(
        r#"
        SELECT
            wd.id,
            wd.user_id,
            u.username,
            u.profile_picture_url,
            COALESCE(wd.workout_start, wd.created_at) as workout_date,
            wd.workout_start,
            wd.workout_end,
            wd.created_at,
            wd.calories_burned,
            wd.duration_minutes,
            wd.activity_name,
            wd.avg_heart_rate,
            wd.max_heart_rate,
            wd.heart_rate_zones,
            COALESCE(wd.stamina_gained, 0.0) as stamina_gained,
            COALESCE(wd.strength_gained, 0.0) as strength_gained,
            COALESCE(wd.stamina_gained, 0.0) + COALESCE(wd.strength_gained, 0.0) as total_points_gained,
            wd.image_url,
            wd.video_url,
            COALESCE(wd.visibility, 'public') as visibility,
            (SELECT COUNT(*) FROM workout_reactions WHERE workout_id = wd.id) as reaction_count,
            (SELECT COUNT(*) FROM workout_comments WHERE workout_id = wd.id) as comment_count,
            EXISTS(
                SELECT 1 FROM workout_reactions
                WHERE workout_id = wd.id AND user_id = $1
            ) as user_has_reacted
        FROM workout_data wd
        INNER JOIN users u ON u.id = wd.user_id
        WHERE
            COALESCE(wd.visibility, 'public') = 'public'
            AND (wd.calories_burned > 100 OR wd.heart_rate_data IS NOT NULL)
            AND ($2::timestamptz IS NULL OR COALESCE(wd.workout_start, wd.created_at) < $2)
        ORDER BY COALESCE(wd.workout_start, wd.created_at) DESC
        LIMIT $3
        "#,
        current_user_id,
        cursor_datetime,
        limit as i64
    )
    .fetch_all(&**pool)
    .await
    {
        Ok(rows) => {
            // Get unique user IDs to check for live game participation
            let user_ids: Vec<Uuid> = rows.iter().map(|row| row.user_id).collect::<std::collections::HashSet<_>>().into_iter().collect();
            
            // Get live game info for each user
            let mut user_live_game_info = std::collections::HashMap::new();
            for user_id in user_ids {
                if let Some(game_info) = get_user_live_game_info(user_id, &**pool).await {
                    user_live_game_info.insert(user_id, game_info);
                }
            }

            rows.into_iter().map(|row| {
                let live_game_info = user_live_game_info.get(&row.user_id).cloned();
                let is_participating = live_game_info.is_some();

                FeedWorkoutItem {
                    id: row.id,
                    user_id: row.user_id,
                    username: row.username,
                    profile_picture_url: row.profile_picture_url,
                    workout_date: row.workout_date.unwrap_or(row.created_at),
                    workout_start: row.workout_start,
                    workout_end: row.workout_end,
                    duration_minutes: row.duration_minutes,
                    calories_burned: row.calories_burned,
                    activity_name: row.activity_name,
                    avg_heart_rate: row.avg_heart_rate,
                    max_heart_rate: row.max_heart_rate,
                    heart_rate_zones: row.heart_rate_zones,
                    stamina_gained: row.stamina_gained.unwrap_or(0.0),
                    strength_gained: row.strength_gained.unwrap_or(0.0),
                    total_points_gained: row.total_points_gained.unwrap_or(0.0),
                    image_url: row.image_url,
                    video_url: row.video_url,
                    reaction_count: row.reaction_count.unwrap_or(0) as i64,
                    comment_count: row.comment_count.unwrap_or(0) as i64,
                    user_has_reacted: row.user_has_reacted.unwrap_or(false),
                    visibility: row.visibility.unwrap_or_else(|| "public".to_string()),
                    is_participating_in_live_game: is_participating,
                    live_game_info,
                }
            }).collect()
        },
        Err(e) => {
            tracing::error!("Failed to fetch newsfeed: {}", e);
            return HttpResponse::InternalServerError().json(json!({
                "error": "Failed to fetch newsfeed"
            }));
        }
    };

    // Get the next cursor from the last item
    let next_cursor = workouts.last().map(|w| w.workout_date.to_rfc3339());
    let has_more = workouts.len() == limit as usize;

    tracing::info!(
        "Successfully retrieved {} feed items for user: {}",
        workouts.len(),
        claims.username
    );

    HttpResponse::Ok().json(json!({
        "success": true,
        "data": {
            "workouts": workouts,
            "pagination": {
                "next_cursor": next_cursor,
                "has_more": has_more,
                "limit": limit,
            }
        }
    }))
}