// src/models/player_pool.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Player pool event types for WebSocket notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerPoolEventType {
    PlayerJoined,      // New player joined the pool
    PlayerLeft,        // Player left the pool (went inactive)
    PlayerAssigned,    // Player was assigned to a team
    PlayerLeftTeam,    // Player left a team and returned to pool
}

impl PlayerPoolEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlayerPoolEventType::PlayerJoined => "player_joined",
            PlayerPoolEventType::PlayerLeft => "player_left",
            PlayerPoolEventType::PlayerAssigned => "player_assigned",
            PlayerPoolEventType::PlayerLeftTeam => "player_left_team",
        }
    }
}

/// Player pool WebSocket event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerPoolEvent {
    pub event_type: PlayerPoolEventType,
    pub user_id: Uuid,
    pub username: String,
    pub league_id: Option<Uuid>,
    pub team_id: Option<Uuid>,      // For PlayerAssigned events
    pub team_name: Option<String>,  // For PlayerAssigned events
    pub timestamp: DateTime<Utc>,
}

/// Player pool entry
#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct PlayerPoolEntry {
    pub id: Uuid,
    pub user_id: Uuid,
    pub league_id: Option<Uuid>,
    pub joined_pool_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
}

/// Request to join player pool
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JoinPlayerPoolRequest {
    pub league_id: Option<Uuid>,
}

/// Response for player pool operations
#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerPoolResponse {
    pub success: bool,
    pub message: String,
    pub entry: Option<PlayerPoolEntry>,
}

/// Player pool list response
#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerPoolListResponse {
    pub entries: Vec<PlayerPoolEntry>,
    pub total_count: usize,
}

/// Filters for querying player pool
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PlayerPoolFilters {
    pub league_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
