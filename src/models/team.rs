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

/// Team member model
#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct TeamMember {
    pub id: Uuid,
    pub team_id: Uuid,
    pub user_id: Uuid,
    pub role: TeamRole,
    pub status: MemberStatus,
    pub joined_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Team member with user information
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamMemberInfo {
    pub id: Uuid,
    pub team_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub email: String,
    pub role: TeamRole,
    pub status: MemberStatus,
    pub joined_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Team role enumeration
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "lowercase")]
pub enum TeamRole {
    #[serde(rename = "owner")]
    Owner,
    #[serde(rename = "admin")]
    Admin,
    #[serde(rename = "member")]
    Member,
}

impl std::fmt::Display for TeamRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeamRole::Owner => write!(f, "owner"),
            TeamRole::Admin => write!(f, "admin"),
            TeamRole::Member => write!(f, "member"),
        }
    }
}

impl std::str::FromStr for TeamRole {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "owner" => Ok(TeamRole::Owner),
            "admin" => Ok(TeamRole::Admin),
            "member" => Ok(TeamRole::Member),
            _ => Err(format!("Invalid team role: {}", s)),
        }
    }
}

/// Member status enumeration
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "lowercase")]
pub enum MemberStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "inactive")]
    Inactive,
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "banned")]
    Banned,
}

impl std::fmt::Display for MemberStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemberStatus::Active => write!(f, "active"),
            MemberStatus::Inactive => write!(f, "inactive"),
            MemberStatus::Pending => write!(f, "pending"),
            MemberStatus::Banned => write!(f, "banned"),
        }
    }
}

impl std::str::FromStr for MemberStatus {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(MemberStatus::Active),
            "inactive" => Ok(MemberStatus::Inactive),
            "pending" => Ok(MemberStatus::Pending),
            "banned" => Ok(MemberStatus::Banned),
            _ => Err(format!("Invalid member status: {}", s)),
        }
    }
}

/// Request to add a user to a team
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AddTeamMemberRequest {
    pub user_id: Option<Uuid>,
    pub username: Option<String>,
    pub email: Option<String>,
    pub role: Option<TeamRole>,
}

/// Request to update a team member
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateTeamMemberRequest {
    pub role: Option<TeamRole>,
    pub status: Option<MemberStatus>,
}

/// Response for team member operations
#[derive(Debug, Serialize, Deserialize)]
pub struct TeamMemberResponse {
    pub success: bool,
    pub message: String,
    pub member: Option<TeamMemberInfo>,
}

/// Team with its members
#[derive(Debug, Serialize, Deserialize)]
pub struct TeamWithMembers {
    pub team: TeamInfo,
    pub members: Vec<TeamMemberInfo>,
    pub member_count: usize,
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

impl AddTeamMemberRequest {
    /// Validate add team member request
    pub fn validate(&self) -> Result<(), String> {
        // Must provide at least one identifier
        if self.user_id.is_none() && self.username.is_none() && self.email.is_none() {
            return Err("Must provide user_id, username, or email to identify the user".to_string());
        }

        // Validate username if provided
        if let Some(username) = &self.username {
            let username = username.trim();
            if username.is_empty() {
                return Err("Username cannot be empty".to_string());
            }
            if username.len() < 2 {
                return Err("Username must be at least 2 characters".to_string());
            }
        }

        // Validate email if provided
        if let Some(email) = &self.email {
            let email = email.trim();
            if email.is_empty() {
                return Err("Email cannot be empty".to_string());
            }
            if !email.contains('@') {
                return Err("Invalid email format".to_string());
            }
        }

        Ok(())
    }
}

impl UpdateTeamMemberRequest {
    /// Validate update team member request
    pub fn validate(&self) -> Result<(), String> {
        // Must provide at least one field to update
        if self.role.is_none() && self.status.is_none() {
            return Err("Must provide at least one field to update".to_string());
        }

        Ok(())
    }
}