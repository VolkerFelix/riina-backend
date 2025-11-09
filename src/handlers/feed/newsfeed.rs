use actix_web::{web, HttpResponse};
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    middleware::auth::Claims,
    models::post::FeedQueryParams,
};

/// Get unified feed with posts (workouts, ads, universal content)
#[tracing::instrument(
    name = "Get unified feed",
    skip(pool, claims),
    fields(username = %claims.username)
)]
pub async fn get_unified_feed(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    query: web::Query<FeedQueryParams>,
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

    // Fetch posts with user info, social counts, and optional workout data
    // Using CTEs to avoid N+1 query problem
    let posts: Vec<serde_json::Value> = match sqlx::query!(
        r#"
        WITH reaction_counts AS (
            SELECT workout_id, COUNT(*) as count
            FROM post_reactions
            WHERE workout_id IS NOT NULL
            GROUP BY workout_id
        ),
        comment_counts AS (
            SELECT workout_id, COUNT(*) as count
            FROM post_comments
            WHERE workout_id IS NOT NULL
            GROUP BY workout_id
        ),
        user_reactions AS (
            SELECT workout_id
            FROM post_reactions
            WHERE user_id = $1 AND workout_id IS NOT NULL
        ),
        effort_ratings AS (
            SELECT workout_data_id, effort_rating
            FROM workout_scoring_feedback
            WHERE user_id = $1
        )
        SELECT
            p.id, p.user_id, p.post_type as "post_type: String",
            p.content, p.workout_id, p.media_urls,
            p.ad_metadata, p.visibility as "visibility: String",
            p.is_editable, p.created_at, p.updated_at, p.edited_at,
            u.username, u.profile_picture_url,

            -- Workout data (if post is workout type)
            wd.workout_start, wd.workout_end,
            wd.duration_minutes, wd.calories_burned, wd.activity_name, wd.user_activity,
            wd.avg_heart_rate, wd.max_heart_rate, wd.heart_rate_zones,
            wd.stamina_gained, wd.strength_gained,
            wd.image_url as workout_image, wd.video_url as workout_video,

            -- Live game info (if workout was part of a live game)
            lse.game_id as live_game_id,
            CASE 
                WHEN lse.team_side = 'home' THEN ht.team_name
                WHEN lse.team_side = 'away' THEN at.team_name
                ELSE NULL
            END as user_team_name,
            CASE 
                WHEN lse.team_side = 'home' THEN at.team_name
                WHEN lse.team_side = 'away' THEN ht.team_name
                ELSE NULL
            END as opponent_team_name,

            -- Social counts (from CTEs - much faster than subqueries)
            COALESCE(rc.count, 0) as reaction_count,
            COALESCE(cc.count, 0) as comment_count,
            (ur.workout_id IS NOT NULL) as user_has_reacted,

            -- Effort rating info
            er.effort_rating as effort_rating,
            CASE
                WHEN p.workout_id IS NOT NULL AND p.user_id = $1 AND er.effort_rating IS NULL THEN true
                ELSE false
            END as "needs_effort_rating!"

        FROM posts p
        JOIN users u ON u.id = p.user_id
        LEFT JOIN workout_data wd ON wd.id = p.workout_id
        LEFT JOIN live_score_events lse ON lse.workout_data_id = wd.id
        LEFT JOIN games g ON g.id = lse.game_id
        LEFT JOIN teams ht ON g.home_team_id = ht.id
        LEFT JOIN teams at ON g.away_team_id = at.id
        LEFT JOIN reaction_counts rc ON rc.workout_id = p.workout_id
        LEFT JOIN comment_counts cc ON cc.workout_id = p.workout_id
        LEFT JOIN user_reactions ur ON ur.workout_id = p.workout_id
        LEFT JOIN effort_ratings er ON er.workout_data_id = p.workout_id
        WHERE
            p.visibility = 'public'
            AND ($2::timestamptz IS NULL OR p.created_at < $2)
        ORDER BY p.created_at DESC
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
            rows.into_iter().map(|row| {
                json!({
                    "id": row.id,
                    "user_id": row.user_id,
                    "username": row.username,
                    "profile_picture_url": row.profile_picture_url,
                    "post_type": row.post_type,
                    "content": row.content,
                    "workout_id": row.workout_id,
                    "media_urls": row.media_urls,
                    "ad_metadata": row.ad_metadata,
                    "visibility": row.visibility,
                    "is_editable": row.is_editable,
                    "created_at": row.created_at,
                    "updated_at": row.updated_at,
                    "edited_at": row.edited_at,

                    // Workout details (null for non-workout posts)
                    "workout_data": if row.workout_id.is_some() {
                        json!({
                            "workout_start": row.workout_start,
                            "workout_end": row.workout_end,
                            "duration_minutes": row.duration_minutes,
                            "calories_burned": row.calories_burned,
                            "activity_name": row.activity_name,
                            "user_activity": row.user_activity,
                            "avg_heart_rate": row.avg_heart_rate,
                            "max_heart_rate": row.max_heart_rate,
                            "heart_rate_zones": row.heart_rate_zones,
                            "stamina_gained": row.stamina_gained,
                            "strength_gained": row.strength_gained,
                            "total_points_gained": row.stamina_gained + row.strength_gained,
                            "image_url": row.workout_image,
                            "video_url": row.workout_video,
                        })
                    } else {
                        json!(null)
                    },

                    // Live game info (if workout was part of a live game)
                    "live_game_info": if row.live_game_id.is_some() && row.user_team_name.is_some() && row.opponent_team_name.is_some() {
                        json!({
                            "game_id": row.live_game_id,
                            "user_team_name": row.user_team_name,
                            "opponent_team_name": row.opponent_team_name,
                        })
                    } else {
                        json!(null)
                    },

                    // Social counts
                    "reaction_count": row.reaction_count.unwrap_or(0),
                    "comment_count": row.comment_count.unwrap_or(0),
                    "user_has_reacted": row.user_has_reacted.unwrap_or(false),

                    // Effort rating (null if not rated, only for current user's workouts)
                    "effort_rating": row.effort_rating,
                    "needs_effort_rating": row.needs_effort_rating,
                })
            }).collect()
        },
        Err(e) => {
            tracing::error!("Failed to fetch unified feed: {}", e);
            return HttpResponse::InternalServerError().json(json!({
                "error": "Failed to fetch feed",
                "details": format!("{}", e)
            }));
        }
    };

    // Get the next cursor from the last item
    let next_cursor = if let Some(last_post) = posts.last() {
        last_post["created_at"].as_str().map(|s| s.to_string())
    } else {
        None
    };
    let has_more = posts.len() == limit as usize;

    tracing::info!(
        "Successfully retrieved {} unified feed items for user: {}",
        posts.len(),
        claims.username
    );

    HttpResponse::Ok().json(json!({
        "success": true,
        "data": {
            "posts": posts,
            "pagination": {
                "next_cursor": next_cursor,
                "has_more": has_more,
                "limit": limit,
            }
        }
    }))
}
