use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum InvitationStatus {
    #[sqlx(rename = "pending")]
    Pending,
    #[sqlx(rename = "accepted")]
    Accepted,
    #[sqlx(rename = "declined")]
    Declined,
    #[sqlx(rename = "expired")]
    Expired,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TeamInvitation {
    pub id: Uuid,
    pub team_id: Uuid,
    pub inviter_id: Uuid,
    pub invitee_id: Uuid,
    pub status: InvitationStatus,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub responded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TeamInvitationWithDetails {
    pub id: Uuid,
    pub team_id: Uuid,
    pub team_name: String,
    pub team_color: String,
    pub inviter_id: Uuid,
    pub inviter_username: String,
    pub invitee_id: Uuid,
    pub invitee_username: String,
    pub status: InvitationStatus,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub responded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct SendInvitationRequest {
    pub invitee_id: Uuid,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RespondToInvitationRequest {
    pub accept: bool,
}
