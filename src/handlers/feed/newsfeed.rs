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
    let posts: Vec<serde_json::Value> = match sqlx::query!(
        r#"
        SELECT
            p.id, p.user_id, p.post_type as "post_type: String",
            p.content, p.workout_id, p.image_urls, p.video_urls,
            p.ad_metadata, p.visibility as "visibility: String",
            p.is_editable, p.created_at, p.updated_at, p.edited_at,
            u.username, u.profile_picture_url,

            -- Workout data (if post is workout type)
            wd.workout_start, wd.workout_end,
            wd.duration_minutes, wd.calories_burned, wd.activity_name,
            wd.avg_heart_rate, wd.max_heart_rate, wd.heart_rate_zones,
            wd.stamina_gained, wd.strength_gained,
            wd.image_url as workout_image, wd.video_url as workout_video,

            -- Social counts (from post_reactions/comments for workout posts)
            COALESCE(
                (SELECT COUNT(*) FROM post_reactions wr WHERE wr.workout_id = p.workout_id),
                0
            ) as reaction_count,
            COALESCE(
                (SELECT COUNT(*) FROM post_comments wc WHERE wc.workout_id = p.workout_id),
                0
            ) as comment_count,
            EXISTS(
                SELECT 1 FROM post_reactions wr
                WHERE wr.workout_id = p.workout_id AND wr.user_id = $1
            ) as user_has_reacted

        FROM posts p
        JOIN users u ON u.id = p.user_id
        LEFT JOIN workout_data wd ON wd.id = p.workout_id
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
                    "image_urls": row.image_urls,
                    "video_urls": row.video_urls,
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

                    // Social counts
                    "reaction_count": row.reaction_count.unwrap_or(0),
                    "comment_count": row.comment_count.unwrap_or(0),
                    "user_has_reacted": row.user_has_reacted.unwrap_or(false),
                })
            }).collect()
        },
        Err(e) => {
            tracing::error!("Failed to fetch unified feed: {}", e);
            return HttpResponse::InternalServerError().json(json!({
                "error": "Failed to fetch feed"
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
