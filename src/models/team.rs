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
    pub league_id: Option<Uuid>,
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
    pub league_id: Option<Uuid>,
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
    pub league_id: Option<Uuid>,
}

/// Request to update team information
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamUpdateRequest {
    pub team_name: Option<String>,
    pub team_description: Option<String>,
    pub team_color: Option<String>,
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

/// Team member request
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamMemberRequest {
    pub user_id: Option<Uuid>,
    pub username: Option<String>,
    pub email: Option<String>,
    pub role: Option<TeamRole>,
}

/// Request to add a user to a team
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AddTeamMemberRequest {
    pub member_request: Vec<TeamMemberRequest>,
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

/// Team info with calculated power
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamInfoWithPower {
    pub id: Uuid,
    pub user_id: Uuid,
    pub team_name: String,
    pub team_description: Option<String>,
    pub team_color: String,
    pub league_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner_username: String,
    pub total_power: f32,
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
            && self.team_color.is_none() {
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

        Ok(())
    }
}

impl AddTeamMemberRequest {
    /// Validate add team member request
    pub fn validate(&self) -> Result<(), String> {
        // Must provide at least one member request
        if self.member_request.is_empty() {
            return Err("Must provide at least one member request".to_string());
        }

        // Validate each member request
        for member in &self.member_request {
            if member.user_id.is_none() && member.username.is_none() && member.email.is_none() {
                return Err("Each member request must provide at least one identifier".to_string());
            }

            if let Some(user_id) = member.user_id {
                if user_id == Uuid::nil() {
                    return Err("Invalid user ID".to_string());
                }
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

/// Team poll for member removal
#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct TeamPoll {
    pub id: Uuid,
    pub team_id: Uuid,
    pub poll_type: PollType,
    pub target_user_id: Uuid,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub status: PollStatus,
    pub result: Option<PollResult>,
    pub executed_at: Option<DateTime<Utc>>,
}

/// Team poll with additional information (internal use, includes creator)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamPollInfo {
    pub id: Uuid,
    pub team_id: Uuid,
    pub team_name: String,
    pub poll_type: PollType,
    pub target_user_id: Uuid,
    pub target_username: String,
    pub created_by: Uuid,
    pub created_by_username: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub status: PollStatus,
    pub result: Option<PollResult>,
    pub executed_at: Option<DateTime<Utc>>,
    pub votes_for: i32,
    pub votes_against: i32,
    pub total_eligible_voters: i32,
}

/// Anonymous poll response (for API responses - creator identity hidden)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnonymousPollInfo {
    pub id: Uuid,
    pub team_id: Uuid,
    pub team_name: String,
    pub poll_type: PollType,
    pub target_user_id: Uuid,
    pub target_username: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub status: PollStatus,
    pub result: Option<PollResult>,
    pub executed_at: Option<DateTime<Utc>>,
    pub votes_for: i32,
    pub votes_against: i32,
    pub total_eligible_voters: i32,
    pub is_creator: bool, // True if current user created this poll
}

impl TeamPollInfo {
    /// Convert to anonymous response, indicating if the given user is the creator
    pub fn to_anonymous(&self, current_user_id: Uuid) -> AnonymousPollInfo {
        AnonymousPollInfo {
            id: self.id,
            team_id: self.team_id,
            team_name: self.team_name.clone(),
            poll_type: self.poll_type.clone(),
            target_user_id: self.target_user_id,
            target_username: self.target_username.clone(),
            created_at: self.created_at,
            expires_at: self.expires_at,
            status: self.status.clone(),
            result: self.result.clone(),
            executed_at: self.executed_at,
            votes_for: self.votes_for,
            votes_against: self.votes_against,
            total_eligible_voters: self.total_eligible_voters,
            is_creator: self.created_by == current_user_id,
        }
    }
}

/// Poll type enumeration
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "snake_case")]
pub enum PollType {
    #[serde(rename = "member_removal")]
    MemberRemoval,
}

impl std::fmt::Display for PollType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PollType::MemberRemoval => write!(f, "member_removal"),
        }
    }
}

impl std::str::FromStr for PollType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "member_removal" => Ok(PollType::MemberRemoval),
            _ => Err(format!("Invalid poll type: {}", s)),
        }
    }
}

/// Poll status enumeration
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "lowercase")]
pub enum PollStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl std::fmt::Display for PollStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PollStatus::Active => write!(f, "active"),
            PollStatus::Completed => write!(f, "completed"),
            PollStatus::Expired => write!(f, "expired"),
            PollStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for PollStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(PollStatus::Active),
            "completed" => Ok(PollStatus::Completed),
            "expired" => Ok(PollStatus::Expired),
            "cancelled" => Ok(PollStatus::Cancelled),
            _ => Err(format!("Invalid poll status: {}", s)),
        }
    }
}

/// Poll result enumeration
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "lowercase")]
pub enum PollResult {
    #[serde(rename = "approved")]
    Approved,
    #[serde(rename = "rejected")]
    Rejected,
    #[serde(rename = "no_consensus")]
    NoConsensus,
}

impl std::fmt::Display for PollResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PollResult::Approved => write!(f, "approved"),
            PollResult::Rejected => write!(f, "rejected"),
            PollResult::NoConsensus => write!(f, "no_consensus"),
        }
    }
}

impl std::str::FromStr for PollResult {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "approved" => Ok(PollResult::Approved),
            "rejected" => Ok(PollResult::Rejected),
            "no_consensus" => Ok(PollResult::NoConsensus),
            _ => Err(format!("Invalid poll result: {}", s)),
        }
    }
}

/// Vote on a team poll
#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct PollVote {
    pub id: Uuid,
    pub poll_id: Uuid,
    pub user_id: Uuid,
    pub vote: VoteChoice,
    pub voted_at: DateTime<Utc>,
}

/// Vote choice enumeration
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "lowercase")]
pub enum VoteChoice {
    #[serde(rename = "for")]
    For,
    #[serde(rename = "against")]
    Against,
}

impl std::fmt::Display for VoteChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VoteChoice::For => write!(f, "for"),
            VoteChoice::Against => write!(f, "against"),
        }
    }
}

impl std::str::FromStr for VoteChoice {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "for" => Ok(VoteChoice::For),
            "against" => Ok(VoteChoice::Against),
            _ => Err(format!("Invalid vote choice: {}", s)),
        }
    }
}

/// Request to create a poll
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreatePollRequest {
    pub poll_type: PollType,
    pub target_user_id: Uuid,
}

/// Request to cast a vote
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CastVoteRequest {
    pub vote: VoteChoice,
}

/// Response for poll operations
#[derive(Debug, Serialize, Deserialize)]
pub struct PollResponse {
    pub success: bool,
    pub message: String,
    pub poll: Option<TeamPollInfo>,
}

impl CreatePollRequest {
    /// Validate create poll request
    pub fn validate(&self) -> Result<(), String> {
        if self.target_user_id == Uuid::nil() {
            return Err("Invalid target user ID".to_string());
        }
        Ok(())
    }
}