use actix_web::{web, HttpResponse};
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;
use std::collections::HashMap;

use crate::{
    middleware::auth::Claims,
    models::post::FeedQueryParams,
};

#[derive(Debug, Clone)]
struct FeedPost {
    id: Uuid,
    user_id: Uuid,
    username: String,
    profile_picture_url: Option<String>,
    post_type: String,
    content: Option<String>,
    workout_id: Option<Uuid>,
    media_urls: Option<serde_json::Value>,
    ad_metadata: Option<serde_json::Value>,
    visibility: String,
    is_editable: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    edited_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
struct WorkoutData {
    workout_start: DateTime<Utc>,
    workout_end: DateTime<Utc>,
    duration_minutes: Option<i32>,
    calories_burned: Option<i32>,
    activity_name: Option<String>,
    user_activity: Option<String>,
    avg_heart_rate: Option<i32>,
    max_heart_rate: Option<i32>,
    heart_rate_zones: Option<serde_json::Value>,
    stamina_gained: f32,
    strength_gained: f32,
    image_url: Option<String>,
    video_url: Option<String>,
}

#[derive(Debug)]
struct LiveGameInfo {
    game_id: Option<Uuid>,
    user_team_name: String,
    opponent_team_name: String,
}

#[derive(Debug)]
struct SocialCounts {
    reaction_count: i64,
    comment_count: i64,
    user_has_reacted: bool,
}

/// Fetch base posts with pagination
async fn fetch_feed_posts(
    pool: &PgPool,
    cursor: Option<DateTime<Utc>>,
    limit: i64,
) -> Result<Vec<FeedPost>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT
            p.id, p.user_id, p.post_type as "post_type: String",
            p.content, p.workout_id, p.media_urls,
            p.ad_metadata, p.visibility as "visibility: String",
            p.is_editable, p.created_at, p.updated_at, p.edited_at,
            u.username, u.profile_picture_url
        FROM posts p
        JOIN users u ON u.id = p.user_id
        WHERE
            p.visibility = 'public'
            AND ($1::timestamptz IS NULL OR p.created_at < $1)
        ORDER BY p.created_at DESC, p.id DESC
        LIMIT $2
        "#,
        cursor,
        limit
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|row| FeedPost {
        id: row.id,
        user_id: row.user_id,
        username: row.username,
        profile_picture_url: row.profile_picture_url,
        post_type: row.post_type,
        content: row.content,
        workout_id: row.workout_id,
        media_urls: row.media_urls,
        ad_metadata: row.ad_metadata,
        visibility: row.visibility,
        is_editable: row.is_editable,
        created_at: row.created_at,
        updated_at: row.updated_at,
        edited_at: row.edited_at,
    }).collect())
}

/// Fetch workout data for given workout IDs
async fn fetch_workout_data(
    pool: &PgPool,
    workout_ids: &[Uuid],
) -> Result<HashMap<Uuid, WorkoutData>, sqlx::Error> {
    if workout_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query!(
        r#"
        SELECT
            id, workout_start, workout_end,
            duration_minutes, calories_burned, activity_name, user_activity,
            avg_heart_rate, max_heart_rate, heart_rate_zones,
            stamina_gained, strength_gained,
            image_url, video_url
        FROM workout_data
        WHERE id = ANY($1)
        "#,
        workout_ids
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|row| (
        row.id,
        WorkoutData {
            workout_start: row.workout_start,
            workout_end: row.workout_end,
            duration_minutes: row.duration_minutes,
            calories_burned: row.calories_burned,
            activity_name: row.activity_name,
            user_activity: row.user_activity,
            avg_heart_rate: row.avg_heart_rate,
            max_heart_rate: row.max_heart_rate,
            heart_rate_zones: row.heart_rate_zones,
            stamina_gained: row.stamina_gained,
            strength_gained: row.strength_gained,
            image_url: row.image_url,
            video_url: row.video_url,
        }
    )).collect())
}

/// Fetch live game info for given workout IDs
async fn fetch_live_game_info(
    pool: &PgPool,
    workout_ids: &[Uuid],
) -> Result<HashMap<Uuid, LiveGameInfo>, sqlx::Error> {
    if workout_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query!(
        r#"
        SELECT
            lse.workout_data_id,
            lse.game_id,
            lse.team_side as "team_side: String",
            ht.team_name as home_team_name,
            at.team_name as away_team_name
        FROM live_score_events lse
        JOIN games g ON g.id = lse.game_id
        JOIN teams ht ON g.home_team_id = ht.id
        JOIN teams at ON g.away_team_id = at.id
        WHERE lse.workout_data_id = ANY($1)
        "#,
        workout_ids
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().filter_map(|row| {
        let user_team_name = if row.team_side == "home" {
            row.home_team_name.clone()
        } else {
            row.away_team_name.clone()
        };

        let opponent_team_name = if row.team_side == "home" {
            row.away_team_name
        } else {
            row.home_team_name
        };

        // Only include if workout_data_id is not None
        row.workout_data_id.map(|workout_id| (workout_id, LiveGameInfo {
            game_id: row.game_id,
            user_team_name,
            opponent_team_name,
        }))
    }).collect())
}

/// Fetch social counts (reactions, comments) for given workout IDs
async fn fetch_social_counts(
    pool: &PgPool,
    workout_ids: &[Uuid],
    user_id: Uuid,
) -> Result<HashMap<Uuid, SocialCounts>, sqlx::Error> {
    if workout_ids.is_empty() {
        return Ok(HashMap::new());
    }

    // Fetch reaction counts
    let reaction_rows = sqlx::query!(
        r#"
        SELECT workout_id, COUNT(*) as "count!"
        FROM post_reactions
        WHERE workout_id = ANY($1)
        GROUP BY workout_id
        "#,
        workout_ids
    )
    .fetch_all(pool)
    .await?;

    let mut reaction_counts: HashMap<Uuid, i64> = reaction_rows.into_iter()
        .map(|row| (row.workout_id, row.count))
        .collect();

    // Fetch comment counts
    let comment_rows = sqlx::query!(
        r#"
        SELECT workout_id, COUNT(*) as "count!"
        FROM post_comments
        WHERE workout_id = ANY($1)
        GROUP BY workout_id
        "#,
        workout_ids
    )
    .fetch_all(pool)
    .await?;

    let mut comment_counts: HashMap<Uuid, i64> = comment_rows.into_iter()
        .map(|row| (row.workout_id, row.count))
        .collect();

    // Fetch user reactions
    let user_reaction_rows = sqlx::query!(
        r#"
        SELECT workout_id
        FROM post_reactions
        WHERE workout_id = ANY($1) AND user_id = $2
        "#,
        workout_ids,
        user_id
    )
    .fetch_all(pool)
    .await?;

    let user_reactions: std::collections::HashSet<Uuid> = user_reaction_rows.into_iter()
        .map(|row| row.workout_id)
        .collect();

    // Combine into SocialCounts
    Ok(workout_ids.iter().map(|&workout_id| {
        (workout_id, SocialCounts {
            reaction_count: reaction_counts.remove(&workout_id).unwrap_or(0),
            comment_count: comment_counts.remove(&workout_id).unwrap_or(0),
            user_has_reacted: user_reactions.contains(&workout_id),
        })
    }).collect())
}

/// Fetch effort ratings for given workout IDs for a user
async fn fetch_effort_ratings(
    pool: &PgPool,
    workout_ids: &[Uuid],
    user_id: Uuid,
) -> Result<HashMap<Uuid, i16>, sqlx::Error> {
    if workout_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query!(
        r#"
        SELECT workout_data_id, effort_rating
        FROM workout_scoring_feedback
        WHERE workout_data_id = ANY($1) AND user_id = $2
        "#,
        workout_ids,
        user_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|row| (row.workout_data_id, row.effort_rating)).collect())
}

/// Calculate engagement score for sorting
fn calculate_engagement_score(
    post: &FeedPost,
    social: Option<&SocialCounts>,
) -> i32 {
    // Only apply engagement ranking for posts within last 5 days
    let now = Utc::now();
    if now.signed_duration_since(post.created_at).num_hours() > 120 {
        return 0;
    }

    let mut score = 0;

    // Media presence: +10 points
    if let Some(media_urls) = &post.media_urls {
        if media_urls.as_array().map(|arr| !arr.is_empty()).unwrap_or(false) {
            score += 10;
        }
    }

    // Social engagement
    if let Some(s) = social {
        score += (s.reaction_count * 2) as i32; // Reactions: 2 points each
        score += (s.comment_count * 3) as i32;  // Comments: 3 points each
    }

    score
}

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

    // Parse cursor if provided (simple format: just timestamp for chronological pagination)
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

    // Step 1: Determine which section we're in
    let now = Utc::now();
    let engagement_cutoff = now - chrono::Duration::hours(120); // 5 days

    // Simple rule: If no cursor, show ranked section. If cursor exists, show chronological.
    let show_ranked_section = cursor_datetime.is_none();

    let mut posts = if show_ranked_section {
        // FIRST REQUEST ONLY: Fetch and rank posts from last 5 days
        // This is a one-time snapshot, never paginated or re-calculated
        let all_recent = match fetch_feed_posts(&pool, None, 1000).await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to fetch feed posts: {}", e);
                return HttpResponse::InternalServerError().json(json!({
                    "error": "Failed to fetch feed"
                }));
            }
        };

        // Filter to only posts within the last 5 days
        all_recent.into_iter()
            .filter(|p| p.created_at >= engagement_cutoff)
            .collect::<Vec<_>>()
    } else {
        // ALL SUBSEQUENT REQUESTS: Pure chronological feed
        match fetch_feed_posts(&pool, cursor_datetime, limit as i64).await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to fetch chronological posts: {}", e);
                return HttpResponse::InternalServerError().json(json!({
                    "error": "Failed to fetch feed"
                }));
            }
        }
    };

    if posts.is_empty() {
        return HttpResponse::Ok().json(json!({
            "success": true,
            "data": {
                "posts": [],
                "pagination": {
                    "next_cursor": null,
                    "has_more": false,
                    "limit": limit,
                }
            }
        }));
    }

    // Step 2: Collect all workout IDs
    let workout_ids: Vec<Uuid> = posts.iter()
        .filter_map(|p| p.workout_id)
        .collect();

    // Step 3: Fetch all related data in parallel
    let (workout_data_result, live_game_result, social_counts_result, effort_ratings_result) = tokio::join!(
        fetch_workout_data(&pool, &workout_ids),
        fetch_live_game_info(&pool, &workout_ids),
        fetch_social_counts(&pool, &workout_ids, current_user_id),
        fetch_effort_ratings(&pool, &workout_ids, current_user_id),
    );

    let workout_data = match workout_data_result {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to fetch workout data: {}", e);
            return HttpResponse::InternalServerError().json(json!({"error": "Failed to fetch workout data"}));
        }
    };

    let live_game_info = match live_game_result {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to fetch live game info: {}", e);
            return HttpResponse::InternalServerError().json(json!({"error": "Failed to fetch live game info"}));
        }
    };

    let social_counts = match social_counts_result {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to fetch social counts: {}", e);
            return HttpResponse::InternalServerError().json(json!({"error": "Failed to fetch social counts"}));
        }
    };

    let effort_ratings = match effort_ratings_result {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to fetch effort ratings: {}", e);
            return HttpResponse::InternalServerError().json(json!({"error": "Failed to fetch effort ratings"}));
        }
    };

    // Step 4: Sort and limit
    if show_ranked_section {
        // Sort by engagement score for the ranked snapshot
        posts.sort_by(|a, b| {
            let a_social = a.workout_id.and_then(|id| social_counts.get(&id));
            let b_social = b.workout_id.and_then(|id| social_counts.get(&id));

            let a_score = calculate_engagement_score(a, a_social);
            let b_score = calculate_engagement_score(b, b_social);

            b_score.cmp(&a_score)
                .then_with(|| b.created_at.cmp(&a.created_at))
                .then_with(|| b.id.cmp(&a.id))
        });

        // Return ALL ranked posts (or up to a reasonable limit like 50)
        // This is the complete ranked section - no pagination
        posts.truncate(50);
    } else {
        // Chronological posts are already sorted by the database query
        // Apply normal pagination limit
        posts.truncate(limit as usize);
    }

    // Step 6: Build response JSON
    let response_posts: Vec<serde_json::Value> = posts.iter().map(|post| {
        let workout_info = post.workout_id.and_then(|id| workout_data.get(&id));
        let game_info = post.workout_id.and_then(|id| live_game_info.get(&id));
        let social = post.workout_id.and_then(|id| social_counts.get(&id));
        let effort = post.workout_id.and_then(|id| effort_ratings.get(&id));

        json!({
            "id": post.id,
            "user_id": post.user_id,
            "username": post.username,
            "profile_picture_url": post.profile_picture_url,
            "post_type": post.post_type,
            "content": post.content,
            "workout_id": post.workout_id,
            "media_urls": post.media_urls,
            "ad_metadata": post.ad_metadata,
            "visibility": post.visibility,
            "is_editable": post.is_editable,
            "created_at": post.created_at,
            "updated_at": post.updated_at,
            "edited_at": post.edited_at,

            "workout_data": workout_info.map(|wd| json!({
                "workout_start": wd.workout_start,
                "workout_end": wd.workout_end,
                "duration_minutes": wd.duration_minutes,
                "calories_burned": wd.calories_burned,
                "activity_name": wd.activity_name,
                "user_activity": wd.user_activity,
                "avg_heart_rate": wd.avg_heart_rate,
                "max_heart_rate": wd.max_heart_rate,
                "heart_rate_zones": wd.heart_rate_zones,
                "stamina_gained": wd.stamina_gained,
                "strength_gained": wd.strength_gained,
                "total_points_gained": wd.stamina_gained + wd.strength_gained,
                "image_url": wd.image_url,
                "video_url": wd.video_url,
            })),

            "live_game_info": game_info.map(|gi| json!({
                "game_id": gi.game_id,
                "user_team_name": gi.user_team_name,
                "opponent_team_name": gi.opponent_team_name,
            })),

            "reaction_count": social.map(|s| s.reaction_count).unwrap_or(0),
            "comment_count": social.map(|s| s.comment_count).unwrap_or(0),
            "user_has_reacted": social.map(|s| s.user_has_reacted).unwrap_or(false),

            "effort_rating": effort,
            "needs_effort_rating": post.workout_id.is_some() && post.user_id == current_user_id && effort.is_none(),
        })
    }).collect();

    // Get the next cursor from the last item
    // For ranked section: use the engagement cutoff so we continue with posts older than 5 days
    // For chronological section: return the last post's timestamp for normal pagination
    let next_cursor = if show_ranked_section && !posts.is_empty() {
        // After ranked section, continue with posts older than the engagement window
        // This prevents duplicates and provides a clean transition
        Some(engagement_cutoff.to_rfc3339())
    } else {
        posts.last().map(|p| p.created_at.to_rfc3339())
    };

    // has_more is true if we returned a full page (only applies to chronological section)
    // For ranked section, we always return true to allow scrolling to chronological posts
    let has_more = if show_ranked_section {
        // Always true after ranked section - there may be chronological posts
        true
    } else {
        // For chronological section, check if we got a full page
        posts.len() == limit as usize
    };

    tracing::info!(
        "Successfully retrieved {} unified feed items for user: {}",
        response_posts.len(),
        claims.username
    );

    HttpResponse::Ok().json(json!({
        "success": true,
        "data": {
            "posts": response_posts,
            "pagination": {
                "next_cursor": next_cursor,
                "has_more": has_more,
                "limit": limit,
            }
        }
    }))
}
