use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// Post type enum matching database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "post_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum PostType {
    Workout,
    Ad,
    Universal,
}

impl PostType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PostType::Workout => "workout",
            PostType::Ad => "ad",
            PostType::Universal => "universal",
        }
    }
}

// Post visibility enum matching database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "post_visibility", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum PostVisibility {
    Public,
    Friends,
    Private,
}

impl PostVisibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            PostVisibility::Public => "public",
            PostVisibility::Friends => "friends",
            PostVisibility::Private => "private",
        }
    }
}

// Core Post model
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Post {
    pub id: Uuid,
    pub user_id: Uuid,
    pub post_type: PostType,
    pub content: Option<String>,
    pub workout_id: Option<Uuid>,
    pub image_urls: Option<Vec<String>>,
    pub video_urls: Option<Vec<String>>,
    pub ad_metadata: Option<serde_json::Value>,
    pub visibility: PostVisibility,
    pub is_editable: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
}

// Post with user info and social counts for feed display
#[derive(Debug, Serialize)]
pub struct PostWithUser {
    #[serde(flatten)]
    pub post: Post,

    // User info
    pub username: String,
    pub profile_picture_url: Option<String>,

    // Social counts
    pub reaction_count: i64,
    pub comment_count: i64,
    pub user_has_reacted: bool,
}

// Create post request
#[derive(Debug, Deserialize)]
pub struct CreatePostRequest {
    pub post_type: PostType,
    pub content: Option<String>,
    pub workout_id: Option<Uuid>, // For workout posts
    pub image_urls: Option<Vec<String>>,
    pub video_urls: Option<Vec<String>>,
    pub visibility: Option<PostVisibility>,
}

// Update post request
#[derive(Debug, Deserialize)]
pub struct UpdatePostRequest {
    pub content: Option<String>,
    pub image_urls: Option<Vec<String>>,
    pub video_urls: Option<Vec<String>>,
    pub visibility: Option<PostVisibility>,
}

// Feed query params
#[derive(Debug, Deserialize)]
pub struct FeedQueryParams {
    pub limit: Option<i32>,
    pub cursor: Option<String>, // ISO 8601 timestamp for cursor-based pagination
}

// Feed response with pagination
#[derive(Debug, Serialize)]
pub struct FeedResponse {
    pub posts: Vec<PostWithUser>,
    pub pagination: FeedPagination,
}

#[derive(Debug, Serialize)]
pub struct FeedPagination {
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub limit: i32,
}
