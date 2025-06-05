// src/models/team.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct Team {
    pub id: Uuid,
    pub user_id: Uuid,
    pub team_name: String,
    pub team_description: Option<String>,
    pub team_color: String,
    pub team_icon: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct TeamInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub team_name: String,
    pub team_description: Option<String>,
    pub team_color: String,
    pub team_icon: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_username: String,
}

/// Request to register a new team
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamRegistrationRequest {
    pub team_name: String,
    pub team_description: Option<String>,
    pub team_color: Option<String>,
    pub team_icon: Option<String>,
}

/// Request to update team information
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamUpdateRequest {
    pub team_name: Option<String>,
    pub team_description: Option<String>,
    pub team_color: Option<String>,
    pub team_icon: Option<String>,
}

/// Response for team registration
#[derive(Debug, Serialize, Deserialize)]
pub struct TeamRegistrationResponse {
    pub success: bool,
    pub message: String,
    pub team_id: Option<Uuid>,
    pub team_name: Option<String>,
}

/// Team with league statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct TeamWithStats {
    pub team: TeamInfo,
    pub league_stats: Option<TeamLeagueStats>,
}

/// Team's league statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct TeamLeagueStats {
    pub current_season_id: Option<Uuid>,
    pub games_played: i32,
    pub wins: i32,
    pub draws: i32,
    pub losses: i32,
    pub points: i32,
    pub position: Option<i32>,
    pub goals_for: i32,
    pub goals_against: i32,
    pub goal_difference: i32,
    pub form: Vec<char>, // Last 5 results: W, D, L
}

/// Team history in leagues
#[derive(Debug, Serialize, Deserialize)]
pub struct TeamSeasonHistory {
    pub season_id: Uuid,
    pub season_name: String,
    pub final_position: Option<i32>,
    pub games_played: i32,
    pub wins: i32,
    pub draws: i32,
    pub losses: i32,
    pub points: i32,
    pub goals_for: i32,
    pub goals_against: i32,
}

impl TeamRegistrationRequest {
    /// Validate team registration request
    pub fn validate(&self) -> Result<(), String> {
        // Validate team name
        let name = self.team_name.trim();
        if name.is_empty() {
            return Err("Team name cannot be empty".to_string());
        }
        
        if name.len() < 2 {
            return Err("Team name must be at least 2 characters".to_string());
        }
        
        if name.len() > 50 {
            return Err("Team name cannot exceed 50 characters".to_string());
        }

        // Validate team name contains valid characters
        if !name.chars().any(|c| c.is_alphanumeric()) {
            return Err("Team name must contain at least one letter or number".to_string());
        }

        // Check for inappropriate content (basic check)
        let lowercase_name = name.to_lowercase();
        let system_reserved = ["admin", "system", "null", "undefined", "root"];
        for word in system_reserved {
            if lowercase_name.contains(word) {
                return Err("Team name contains reserved word".to_string());
            }
        }

        // Validate team description if provided
        if let Some(desc) = &self.team_description {
            if desc.len() > 500 {
                return Err("Team description cannot exceed 500 characters".to_string());
            }
        }

        // Validate team color if provided
        if let Some(color) = &self.team_color {
            if !color.starts_with('#') || color.len() != 7 {
                return Err("Team color must be a valid hex color (e.g., #FF0000)".to_string());
            }
            
            // Check if it's a valid hex string
            if !color[1..].chars().all(|c| c.is_ascii_hexdigit()) {
                return Err("Team color must be a valid hex color".to_string());
            }
        }

        // Validate team icon if provided
        if let Some(icon) = &self.team_icon {
            if icon.len() > 10 {
                return Err("Team icon must be 10 characters or less".to_string());
            }
        }

        Ok(())
    }

    /// Get sanitized team name
    pub fn get_sanitized_name(&self) -> String {
        self.team_name
            .trim()
            .chars()
            .filter(|&c| c != '\0' && c != '\t' && c != '\r' && c != '\n')
            .collect::<String>()
            .trim()
            .to_string()
    }
}

impl TeamUpdateRequest {
    /// Validate team update request
    pub fn validate(&self) -> Result<(), String> {
        // Check if at least one field is being updated
        if self.team_name.is_none() 
            && self.team_description.is_none() 
            && self.team_color.is_none() 
            && self.team_icon.is_none() {
            return Err("At least one field must be provided for update".to_string());
        }

        // Validate team name if provided
        if let Some(name) = &self.team_name {
            let name = name.trim();
            if name.is_empty() {
                return Err("Team name cannot be empty".to_string());
            }
            
            if name.len() < 2 {
                return Err("Team name must be at least 2 characters".to_string());
            }
            
            if name.len() > 50 {
                return Err("Team name cannot exceed 50 characters".to_string());
            }

            if !name.chars().any(|c| c.is_alphanumeric()) {
                return Err("Team name must contain at least one letter or number".to_string());
            }
        }

        // Validate team description if provided
        if let Some(desc) = &self.team_description {
            if desc.len() > 500 {
                return Err("Team description cannot exceed 500 characters".to_string());
            }
        }

        // Validate team color if provided
        if let Some(color) = &self.team_color {
            if !color.starts_with('#') || color.len() != 7 {
                return Err("Team color must be a valid hex color (e.g., #FF0000)".to_string());
            }
            
            if !color[1..].chars().all(|c| c.is_ascii_hexdigit()) {
                return Err("Team color must be a valid hex color".to_string());
            }
        }

        // Validate team icon if provided
        if let Some(icon) = &self.team_icon {
            if icon.len() > 10 {
                return Err("Team icon must be 10 characters or less".to_string());
            }
        }

        Ok(())
    }
}