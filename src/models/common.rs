use serde::{Deserialize, Serialize};
use std::fmt::Display;
use uuid::Uuid;

/// Generic API response wrapper used across all handlers
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    /// Create a successful response with data
    pub fn success(message: impl Into<String>, data: T) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
            error: None,
        }
    }

    /// Create a successful response without data
    pub fn success_message(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
            error: None,
        }
    }

    /// Create an error response
    pub fn error(message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            success: false,
            message: msg.clone(),
            data: None,
            error: Some(msg),
        }
    }

    /// Create an error response with custom error message
    pub fn error_with_message(message: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
            error: Some(error.into()),
        }
    }
}

/// Common player statistics used across different contexts
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlayerStats {
    pub stamina: i32,
    pub strength: i32,
}

impl PlayerStats {
    pub fn new(stamina: i32, strength: i32) -> Self {
        Self { stamina, strength }
    }

    /// Calculate total power (used in game evaluations)
    pub fn total_power(&self) -> i32 {
        self.stamina + self.strength
    }
}

/// Represents changes to player statistics
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatChanges {
    pub stamina_change: i32,
    pub strength_change: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

impl StatChanges {
    pub fn new(stamina_change: i32, strength_change: i32) -> Self {
        Self {
            stamina_change,
            strength_change,
            reasoning: None,
        }
    }

    pub fn with_reasoning(mut self, reasoning: impl Into<String>) -> Self {
        self.reasoning = Some(reasoning.into());
        self
    }
}

/// Common match result enum used across game-related modules
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchResult {
    Win,
    Loss,
    Draw,
}

impl MatchResult {
    /// Get the inverse result (from opponent's perspective)
    pub fn inverse(&self) -> Self {
        match self {
            MatchResult::Win => MatchResult::Loss,
            MatchResult::Loss => MatchResult::Win,
            MatchResult::Draw => MatchResult::Draw,
        }
    }
}

impl Display for MatchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Unified team standings structure used across different contexts
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamStandings {
    pub team_id: Uuid,
    pub team_name: String,
    pub team_color: Option<String>,
    pub position: u32,
    pub games_played: u32,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub points: u32,
    pub position_change: Option<i32>, // +1 = moved up, -1 = moved down, 0 = no change
}

impl TeamStandings {
    /// Calculate win percentage
    pub fn win_percentage(&self) -> f64 {
        if self.games_played == 0 {
            0.0
        } else {
            (self.wins as f64 / self.games_played as f64) * 100.0
        }
    }

    /// Calculate points per game
    pub fn points_per_game(&self) -> f64 {
        if self.games_played == 0 {
            0.0
        } else {
            self.points as f64 / self.games_played as f64
        }
    }
}