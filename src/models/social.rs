use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum ReactionType {
    #[serde(rename = "fire")]
    Fire,
}

impl ReactionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReactionType::Fire => "fire",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "fire" => Some(ReactionType::Fire),
            _ => None,
        }
    }
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct WorkoutReaction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workout_id: Uuid,
    pub reaction_type: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WorkoutReactionWithUser {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub reaction_type: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReactionRequest {
    pub reaction_type: String,
}

#[derive(Debug, Serialize)]
pub struct ReactionSummary {
    pub reaction_type: String,
    pub count: i64,
    pub user_reacted: bool,
}

#[derive(Debug, Serialize)]
pub struct WorkoutReactionSummary {
    pub workout_id: Uuid,
    pub fire_count: i64,
    pub user_reacted: bool,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct WorkoutComment {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workout_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub content: String,
    pub is_edited: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkoutCommentWithUser {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub workout_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub content: String,
    pub is_edited: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub replies: Vec<WorkoutCommentWithUser>,
    pub fire_count: i64,
    pub user_reacted: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub content: String,
    pub parent_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCommentRequest {
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct CommentListResponse {
    pub comments: Vec<WorkoutCommentWithUser>,
    pub total_count: i64,
    pub page: i32,
    pub per_page: i32,
}

#[derive(Debug, Deserialize)]
pub struct CommentQueryParams {
    pub page: Option<i32>,
    pub per_page: Option<i32>,
}

// Comment Reaction Models
#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct CommentReaction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub comment_id: Uuid,
    pub reaction_type: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CommentReactionWithUser {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub reaction_type: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CommentReactionSummary {
    pub comment_id: Uuid,
    pub fire_count: i64,
    pub user_reacted: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentReactionRequest {
    pub reaction_type: String,
}

// Notification Models
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotificationType {
    Reaction,
    Comment,
    Reply,
}

impl NotificationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NotificationType::Reaction => "reaction",
            NotificationType::Comment => "comment",
            NotificationType::Reply => "reply",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NotificationWithUser {
    pub id: Uuid,
    pub recipient_id: Option<Uuid>,
    pub actor_id: Uuid,
    pub actor_username: String,
    pub notification_type: String,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub message: String,
    pub read: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct NotificationListResponse {
    pub notifications: Vec<NotificationWithUser>,
    pub total_count: i64,
    pub unread_count: i64,
    pub page: i32,
    pub per_page: i32,
}

#[derive(Debug, Deserialize)]
pub struct NotificationQueryParams {
    pub page: Option<i32>,
    pub per_page: Option<i32>,
    pub unread_only: Option<bool>,
}