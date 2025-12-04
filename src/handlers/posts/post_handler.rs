use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    middleware::auth::Claims,
    models::post::{CreatePostRequest, UpdatePostRequest, PostType, PostVisibility},
    models::common::ApiResponse,
};

/// Create a new post
#[tracing::instrument(
    name = "Create post",
    skip(pool, claims, body),
    fields(username = %claims.username)
)]
pub async fn create_post(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    body: web::Json<CreatePostRequest>,
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::BadRequest().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };

    // Validate workout post has workout_id
    if body.post_type == PostType::Workout && body.workout_id.is_none() {
        return HttpResponse::BadRequest().json(
            ApiResponse::<()>::error("Workout posts must have a workout_id")
        );
    }

    // Verify workout belongs to user if workout_id provided
    if let Some(workout_id) = body.workout_id {
        match sqlx::query!(
            "SELECT user_id FROM workout_data WHERE id = $1",
            workout_id
        )
        .fetch_optional(&**pool)
        .await
        {
            Ok(Some(workout)) => {
                if workout.user_id != user_id {
                    return HttpResponse::Forbidden().json(
                        ApiResponse::<()>::error("You can only create posts for your own workouts")
                    );
                }
            }
            Ok(None) => {
                return HttpResponse::NotFound().json(
                    ApiResponse::<()>::error("Workout not found")
                );
            }
            Err(e) => {
                tracing::error!("Database error: {}", e);
                return HttpResponse::InternalServerError().json(
                    ApiResponse::<()>::error("Database error")
                );
            }
        }
    }

    let visibility = body.visibility.clone().unwrap_or(PostVisibility::Public);

    // Convert media_urls to JSON
    let media_urls_json = body.media_urls.as_ref().map(|media| {
        serde_json::to_value(media).unwrap_or(serde_json::Value::Null)
    });

    // Insert post
    let post_id = Uuid::new_v4();
    let post_type_str = body.post_type.as_str();
    let visibility_str = visibility.as_str();

    let result = sqlx::query(
        r#"
        INSERT INTO posts (
            id, user_id, post_type, content, workout_id,
            media_urls, visibility
        )
        VALUES ($1, $2, $3::post_type, $4, $5, $6, $7::post_visibility)
        RETURNING id
        "#
    )
    .bind(post_id)
    .bind(user_id)
    .bind(post_type_str)
    .bind(body.content.as_ref())
    .bind(body.workout_id)
    .bind(media_urls_json)
    .bind(visibility_str)
    .fetch_one(&**pool)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("Created post {} for user {}", post_id, claims.username);
            HttpResponse::Ok().json(json!({
                "success": true,
                "data": {
                    "id": post_id
                }
            }))
        }
        Err(e) => {
            tracing::error!("Failed to create post: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to create post")
            )
        }
    }
}

/// Update an existing post
#[tracing::instrument(
    name = "Update post",
    skip(pool, claims, body),
    fields(username = %claims.username, post_id = %post_id)
)]
pub async fn update_post(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    post_id: web::Path<Uuid>,
    body: web::Json<UpdatePostRequest>,
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::BadRequest().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };

    let post_id = post_id.into_inner();

    // Check if post exists and belongs to user
    let post = match sqlx::query_as::<_, (Uuid, bool, String, Option<Uuid>)>(
        "SELECT user_id, is_editable, post_type::text, workout_id FROM posts WHERE id = $1"
    )
    .bind(post_id)
    .fetch_optional(&**pool)
    .await
    {
        Ok(Some((user_id_db, is_editable, post_type, workout_id))) => (user_id_db, is_editable, post_type, workout_id),
        Ok(None) => {
            return HttpResponse::NotFound().json(
                ApiResponse::<()>::error("Post not found")
            );
        }
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Database error")
            );
        }
    };

    // Verify ownership
    if post.0 != user_id {
        return HttpResponse::Forbidden().json(
            ApiResponse::<()>::error("You can only edit your own posts")
        );
    }

    // Check if editable
    if !post.1 {
        return HttpResponse::Forbidden().json(
            ApiResponse::<()>::error("This post cannot be edited")
        );
    }

    // Ad posts cannot be edited by users
    if post.2 == "ad" {
        return HttpResponse::Forbidden().json(
            ApiResponse::<()>::error("Ad posts cannot be edited")
        );
    }

    // Build update query dynamically based on provided fields
    let now = Utc::now();
    let visibility_str = body.visibility.as_ref().map(|v| v.as_str());

    // Convert media_urls to JSON
    // Important: We need to distinguish between "not updating media" (None) and "clearing media" (Some(empty array))
    let media_urls_json = match &body.media_urls {
        Some(media) => {
            // If media array is provided (even if empty), convert it to JSON
            Some(serde_json::to_value(media).unwrap_or(serde_json::Value::Null))
        }
        None => {
            // If media_urls field is not provided at all, don't update it
            None
        }
    };

    // Build the UPDATE query
    // Note: Only update media_urls if it's explicitly provided (even if empty)
    let result = if body.media_urls.is_some() {
        // Media update is requested
        sqlx::query(
            r#"
            UPDATE posts
            SET
                content = COALESCE($1, content),
                media_urls = $2,
                visibility = COALESCE($3::post_visibility, visibility),
                updated_at = $4,
                edited_at = $4
            WHERE id = $5
            "#
        )
        .bind(body.content.as_ref())
        .bind(media_urls_json)
        .bind(visibility_str)
        .bind(now)
        .bind(post_id)
        .execute(&**pool)
        .await
    } else {
        // No media update requested
        sqlx::query(
            r#"
            UPDATE posts
            SET
                content = COALESCE($1, content),
                visibility = COALESCE($2::post_visibility, visibility),
                updated_at = $3,
                edited_at = $3
            WHERE id = $4
            "#
        )
        .bind(body.content.as_ref())
        .bind(visibility_str)
        .bind(now)
        .bind(post_id)
        .execute(&**pool)
        .await
    };

    // If it's a workout post and activity_name is provided, update the workout
    // Store user-edited activity in user_activity field (not activity_name which is read-only)
    if post.2 == "workout" && body.activity_name.is_some() {
        if let Some(workout_id) = post.3 {
            let _ = sqlx::query(
                r#"
                UPDATE workout_data
                SET user_activity = $1,
                    updated_at = NOW()
                WHERE id = $2
                "#
            )
            .bind(body.activity_name.as_ref())
            .bind(workout_id)
            .execute(&**pool)
            .await;
        }
    }

    match result {
        Ok(_) => {
            tracing::info!("Updated post {} for user {}", post_id, claims.username);
            HttpResponse::Ok().json(json!({
                "success": true,
                "message": "Post updated successfully"
            }))
        }
        Err(e) => {
            tracing::error!("Failed to update post: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to update post")
            )
        }
    }
}

/// Delete a post
#[tracing::instrument(
    name = "Delete post",
    skip(pool, claims),
    fields(username = %claims.username, post_id = %post_id)
)]
pub async fn delete_post(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    post_id: web::Path<Uuid>,
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::BadRequest().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };

    let post_id = post_id.into_inner();

    // Check if post exists and belongs to user
    let post_user_id = match sqlx::query_as::<_, (Uuid,)>(
        "SELECT user_id FROM posts WHERE id = $1"
    )
    .bind(post_id)
    .fetch_optional(&**pool)
    .await
    {
        Ok(Some((user_id_db,))) => user_id_db,
        Ok(None) => {
            return HttpResponse::NotFound().json(
                ApiResponse::<()>::error("Post not found")
            );
        }
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Database error")
            );
        }
    };

    // Verify ownership
    if post_user_id != user_id {
        return HttpResponse::Forbidden().json(
            ApiResponse::<()>::error("You can only delete your own posts")
        );
    }

    // Delete the post (CASCADE will handle reactions/comments)
    match sqlx::query("DELETE FROM posts WHERE id = $1")
        .bind(post_id)
        .execute(&**pool)
        .await
    {
        Ok(_) => {
            tracing::info!("Deleted post {} for user {}", post_id, claims.username);
            HttpResponse::Ok().json(json!({
                "success": true,
                "message": "Post deleted successfully"
            }))
        }
        Err(e) => {
            tracing::error!("Failed to delete post: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to delete post")
            )
        }
    }
}

/// Get a single post by ID
#[tracing::instrument(
    name = "Get post",
    skip(pool, claims),
    fields(username = %claims.username, post_id = %post_id)
)]
pub async fn get_post(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    post_id: web::Path<Uuid>,
) -> HttpResponse {
    let _current_user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::BadRequest().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };

    let post_id = post_id.into_inner();

    // Fetch post with workout data if applicable
    let result = sqlx::query(
        r#"
        SELECT
            p.id, p.user_id, p.post_type::text, p.content, p.workout_id,
            p.media_urls, p.ad_metadata, p.visibility::text,
            p.is_editable, p.created_at, p.updated_at, p.edited_at,
            u.username, u.profile_picture_url,
            wd.workout_start, wd.workout_end, wd.duration_minutes,
            wd.calories_burned, wd.activity_name, wd.user_activity, wd.avg_heart_rate,
            wd.max_heart_rate, wd.heart_rate_zones, wd.stamina_gained,
            wd.strength_gained, wd.total_points_gained,
            wd.image_url as workout_image_url, wd.video_url as workout_video_url
        FROM posts p
        JOIN users u ON u.id = p.user_id
        LEFT JOIN workout_data wd ON wd.id = p.workout_id
        WHERE p.id = $1
        "#
    )
    .bind(post_id)
    .fetch_optional(&**pool)
    .await;

    match result {
        Ok(Some(row)) => {
            // Build workout_data if this is a workout post
            let workout_data = if row.try_get::<Option<Uuid>, _>("workout_id").ok().flatten().is_some() {
                Some(json!({
                    "workout_start": row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("workout_start").ok().flatten(),
                    "workout_end": row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("workout_end").ok().flatten(),
                    "duration_minutes": row.try_get::<Option<i32>, _>("duration_minutes").ok().flatten(),
                    "calories_burned": row.try_get::<Option<i32>, _>("calories_burned").ok().flatten(),
                    "activity_name": row.try_get::<Option<String>, _>("activity_name").ok().flatten(),
                    "user_activity": row.try_get::<Option<String>, _>("user_activity").ok().flatten(),
                    "avg_heart_rate": row.try_get::<Option<f32>, _>("avg_heart_rate").ok().flatten(),
                    "max_heart_rate": row.try_get::<Option<f32>, _>("max_heart_rate").ok().flatten(),
                    "heart_rate_zones": row.try_get::<Option<serde_json::Value>, _>("heart_rate_zones").ok().flatten(),
                    "stamina_gained": row.try_get::<f32, _>("stamina_gained").ok(),
                    "strength_gained": row.try_get::<f32, _>("strength_gained").ok(),
                    "total_points_gained": row.try_get::<i32, _>("total_points_gained").ok(),
                    "image_url": row.try_get::<Option<String>, _>("workout_image_url").ok().flatten(),
                    "video_url": row.try_get::<Option<String>, _>("workout_video_url").ok().flatten(),
                }))
            } else {
                None
            };

            // Build JSON response manually from row
            HttpResponse::Ok().json(json!({
                "success": true,
                "data": {
                    "id": row.try_get::<Uuid, _>("id").ok(),
                    "user_id": row.try_get::<Uuid, _>("user_id").ok(),
                    "username": row.try_get::<String, _>("username").ok(),
                    "profile_picture_url": row.try_get::<Option<String>, _>("profile_picture_url").ok().flatten(),
                    "post_type": row.try_get::<String, _>("post_type").ok(),
                    "content": row.try_get::<Option<String>, _>("content").ok().flatten(),
                    "workout_id": row.try_get::<Option<Uuid>, _>("workout_id").ok().flatten(),
                    "workout_data": workout_data,
                    "media_urls": row.try_get::<Option<serde_json::Value>, _>("media_urls").ok().flatten(),
                    "ad_metadata": row.try_get::<Option<serde_json::Value>, _>("ad_metadata").ok().flatten(),
                    "visibility": row.try_get::<String, _>("visibility").ok(),
                    "is_editable": row.try_get::<bool, _>("is_editable").ok(),
                    "created_at": row.try_get::<chrono::DateTime<chrono::Utc>, _>("created_at").ok(),
                    "updated_at": row.try_get::<chrono::DateTime<chrono::Utc>, _>("updated_at").ok(),
                    "edited_at": row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("edited_at").ok().flatten(),
                    "reaction_count": 0,
                    "comment_count": 0,
                    "user_has_reacted": false
                }
            }))
        }
        Ok(None) => {
            HttpResponse::NotFound().json(
                ApiResponse::<()>::error("Post not found")
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch post: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to fetch post")
            )
        }
    }
}

/// Get a post by workout ID
#[tracing::instrument(
    name = "Get post by workout ID",
    skip(pool, claims),
    fields(username = %claims.username, workout_id = %workout_id)
)]
pub async fn get_post_by_workout_id(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    workout_id: web::Path<Uuid>,
) -> HttpResponse {
    let current_user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::BadRequest().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };

    let workout_id = workout_id.into_inner();

    // Fetch post with workout data
    let result = sqlx::query(
        r#"
        SELECT
            p.id, p.user_id, p.post_type::text, p.content, p.workout_id,
            p.media_urls, p.ad_metadata, p.visibility::text,
            p.is_editable, p.created_at, p.updated_at, p.edited_at,
            u.username, u.profile_picture_url,
            wd.workout_start, wd.workout_end, wd.duration_minutes,
            wd.calories_burned, wd.activity_name, wd.avg_heart_rate,
            wd.max_heart_rate, wd.heart_rate_zones, wd.stamina_gained,
            wd.strength_gained, wd.total_points_gained,
            wd.image_url as workout_image_url, wd.video_url as workout_video_url
        FROM posts p
        JOIN users u ON u.id = p.user_id
        LEFT JOIN workout_data wd ON wd.id = p.workout_id
        WHERE p.workout_id = $1 AND p.user_id = $2
        "#
    )
    .bind(workout_id)
    .bind(current_user_id)
    .fetch_optional(&**pool)
    .await;

    match result {
        Ok(Some(row)) => {
            // Build workout data if available
            let workout_data = if row.try_get::<Option<Uuid>, _>("workout_id").ok().flatten().is_some() {
                Some(json!({
                    "workout_start": row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("workout_start").ok().flatten(),
                    "workout_end": row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("workout_end").ok().flatten(),
                    "duration_minutes": row.try_get::<Option<i32>, _>("duration_minutes").ok().flatten(),
                    "calories_burned": row.try_get::<Option<f64>, _>("calories_burned").ok().flatten(),
                    "activity_name": row.try_get::<Option<String>, _>("activity_name").ok().flatten(),
                    "avg_heart_rate": row.try_get::<Option<f64>, _>("avg_heart_rate").ok().flatten(),
                    "max_heart_rate": row.try_get::<Option<f64>, _>("max_heart_rate").ok().flatten(),
                    "heart_rate_zones": row.try_get::<Option<serde_json::Value>, _>("heart_rate_zones").ok().flatten(),
                    "stamina_gained": row.try_get::<Option<f64>, _>("stamina_gained").ok().flatten(),
                    "strength_gained": row.try_get::<Option<f64>, _>("strength_gained").ok().flatten(),
                    "image_url": row.try_get::<Option<String>, _>("workout_image_url").ok().flatten(),
                    "video_url": row.try_get::<Option<String>, _>("workout_video_url").ok().flatten()
                }))
            } else {
                None
            };

            // Build JSON response manually from row
            HttpResponse::Ok().json(json!({
                "success": true,
                "data": {
                    "id": row.try_get::<Uuid, _>("id").ok(),
                    "user_id": row.try_get::<Uuid, _>("user_id").ok(),
                    "username": row.try_get::<String, _>("username").ok(),
                    "profile_picture_url": row.try_get::<Option<String>, _>("profile_picture_url").ok().flatten(),
                    "post_type": row.try_get::<String, _>("post_type").ok(),
                    "content": row.try_get::<Option<String>, _>("content").ok().flatten(),
                    "workout_id": row.try_get::<Option<Uuid>, _>("workout_id").ok().flatten(),
                    "workout_data": workout_data,
                    "media_urls": row.try_get::<Option<serde_json::Value>, _>("media_urls").ok().flatten(),
                    "ad_metadata": row.try_get::<Option<serde_json::Value>, _>("ad_metadata").ok().flatten(),
                    "visibility": row.try_get::<String, _>("visibility").ok(),
                    "is_editable": row.try_get::<bool, _>("is_editable").ok(),
                    "created_at": row.try_get::<chrono::DateTime<chrono::Utc>, _>("created_at").ok(),
                    "updated_at": row.try_get::<chrono::DateTime<chrono::Utc>, _>("updated_at").ok(),
                    "edited_at": row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("edited_at").ok().flatten(),
                    "reaction_count": 0,
                    "comment_count": 0,
                    "user_has_reacted": false
                }
            }))
        }
        Ok(None) => {
            HttpResponse::NotFound().json(
                ApiResponse::<()>::error("Post not found for this workout")
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch post by workout ID: {}", e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to fetch post")
            )
        }
    }
}
