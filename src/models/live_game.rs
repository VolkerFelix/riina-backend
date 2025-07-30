use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Live game state with detailed scoring and player contributions
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LiveGame {
    pub id: Uuid,
    pub game_id: Uuid,
    pub home_team_id: Uuid,
    pub home_team_name: String,
    pub away_team_id: Uuid,
    pub away_team_name: String,
    pub home_score: i32,
    pub away_score: i32,
    pub home_power: i32,
    pub away_power: i32,
    pub game_start_time: DateTime<Utc>,
    pub game_end_time: DateTime<Utc>,
    pub last_score_time: Option<DateTime<Utc>>,
    pub last_scorer_id: Option<Uuid>,
    pub last_scorer_name: Option<String>,
    pub last_scorer_team: Option<String>, // "home" or "away"
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl LiveGame {
    /// Calculate game progress as percentage (0-100)
    pub fn game_progress(&self) -> f32 {
        let now = Utc::now();
        if now < self.game_start_time {
            return 0.0;
        }
        if now >= self.game_end_time {
            return 100.0;
        }
        
        let total_duration = self.game_end_time - self.game_start_time;
        let elapsed_duration = now - self.game_start_time;
        
        if total_duration.num_seconds() > 0 {
            ((elapsed_duration.num_seconds() as f32) / (total_duration.num_seconds() as f32) * 100.0).min(100.0)
        } else {
            0.0
        }
    }

    /// Get time remaining in human readable format
    pub fn time_remaining(&self) -> Option<String> {
        let now = Utc::now();
        if now >= self.game_end_time || !self.is_active {
            return Some("Final".to_string());
        }

        let remaining = self.game_end_time - now;
        let hours = remaining.num_hours();
        let minutes = remaining.num_minutes() % 60;

        if hours > 0 {
            Some(format!("{}h {}m", hours, minutes))
        } else if minutes > 0 {
            Some(format!("{}m", minutes))
        } else {
            Some("< 1m".to_string())
        }
    }

    /// Check if game should be marked as finished
    pub fn should_finish(&self) -> bool {
        Utc::now() >= self.game_end_time
    }
}

/// Player contribution in a live game
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LivePlayerContribution {
    pub id: Uuid,
    pub live_game_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub team_id: Uuid,
    pub team_name: String,
    pub team_side: String, // "home" or "away"
    pub current_power: i32,
    pub total_score_contribution: i32,
    pub last_contribution_time: Option<DateTime<Utc>>,
    pub contribution_count: i32, // Number of workout uploads during game
    pub is_currently_active: bool, // Has contributed in last 30 minutes
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl LivePlayerContribution {
    /// Check if player is recently active (contributed in last 30 minutes)
    pub fn is_recently_active(&self) -> bool {
        if let Some(last_contribution) = self.last_contribution_time {
            let thirty_minutes_ago = Utc::now() - chrono::Duration::minutes(30);
            last_contribution > thirty_minutes_ago
        } else {
            false
        }
    }

    /// Update player's recent activity status
    pub fn update_activity_status(&mut self) {
        self.is_currently_active = self.is_recently_active();
    }
}

/// Score event that happened during a live game
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LiveScoreEvent {
    pub id: Uuid,
    pub live_game_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub team_id: Uuid,
    pub team_side: String, // "home" or "away"
    pub score_points: i32,
    pub power_contribution: i32,
    pub stamina_gained: i32,
    pub strength_gained: i32,
    pub description: String,
    pub occurred_at: DateTime<Utc>,
}

/// Request to create a new live game
#[derive(Debug, Deserialize)]
pub struct CreateLiveGameRequest {
    pub game_id: Uuid,
}

/// Response when creating or updating a live game
#[derive(Debug, Serialize)]
pub struct LiveGameResponse {
    pub live_game: LiveGame,
    pub home_contributions: Vec<LivePlayerContribution>,
    pub away_contributions: Vec<LivePlayerContribution>,
    pub recent_events: Vec<LiveScoreEvent>,
}

/// Update to a live game score
#[derive(Debug, Serialize, Deserialize)]
pub struct LiveGameScoreUpdate {
    pub user_id: Uuid,
    pub username: String,
    pub team_id: Uuid,
    pub score_increase: i32,
    pub power_increase: i32,
    pub stamina_gained: i32,
    pub strength_gained: i32,
    pub description: String,
}

/// Live game summary for API responses
#[derive(Debug, Serialize)]
pub struct LiveGameSummary {
    pub game_id: Uuid,
    pub home_team: TeamSummary,
    pub away_team: TeamSummary,
    pub game_progress: f32,
    pub time_remaining: Option<String>,
    pub is_active: bool,
    pub last_scorer: Option<ScorerInfo>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct TeamSummary {
    pub team_id: Uuid,
    pub team_name: String,
    pub score: i32,
    pub power: i32,
    pub active_players: i32,
    pub recent_contributors: Vec<PlayerSummary>,
}

#[derive(Debug, Serialize)]
pub struct PlayerSummary {
    pub user_id: Uuid,
    pub username: String,
    pub power_contribution: i32,
    pub is_recently_active: bool,
    pub last_contribution: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct ScorerInfo {
    pub user_id: Uuid,
    pub username: String,
    pub team_name: String,
    pub team_side: String,
    pub points_scored: i32,
    pub scored_at: DateTime<Utc>,
}

/// Statistics for a live game
#[derive(Debug, Serialize)]
pub struct LiveGameStats {
    pub total_score_events: i32,
    pub total_players_active: i32,
    pub average_power_per_team: f32,
    pub most_active_player: Option<PlayerSummary>,
    pub highest_single_contribution: Option<LiveScoreEvent>,
    pub game_duration_minutes: i64,
}