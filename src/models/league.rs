// src/models/league.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use std::fmt;

// Live game score update structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LiveGameScoreUpdate {
    pub user_id: Uuid,
    pub username: String,
    pub score_increase: f32,
}

#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct LeagueSeason {
    pub id: Uuid,
    pub league_id: Uuid,
    pub name: String,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub evaluation_cron: Option<String>, // Cron expression for when to evaluate games
    pub evaluation_timezone: Option<String>, // Timezone for evaluation (e.g., "UTC", "America/New_York")
    pub auto_evaluation_enabled: Option<bool>, // Whether automatic evaluation is enabled
    pub game_duration_seconds: i64, // Duration of each game in seconds (default: 518400 = 6 days)
    pub games_per_matchup: Option<i32>, // Number of games per team matchup: 1 = single round-robin, 2 = double round-robin (default: 2)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EnhancedLeagueSeason {
    pub id: String,
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    pub total_teams: i32,
    pub current_week: i32,
    pub total_weeks: i32,
}

#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct LeagueGame {
    pub id: Uuid,
    pub season_id: Uuid,
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
    pub week_number: i32,
    pub is_first_leg: bool,
    pub status: GameStatus,
    pub winner_team_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // New consolidated fields from live_games table
    pub home_score: i32,
    pub away_score: i32,
    #[serde(default)]
    pub game_start_time: Option<DateTime<Utc>>,
    #[serde(default)]
    pub game_end_time: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_score_time: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_scorer_id: Option<Uuid>,
    #[serde(default)]
    pub last_scorer_name: Option<String>,
    #[serde(default)]
    pub last_scorer_team: Option<String>,
}

impl LeagueGame {
    /// Create a LeagueGame with all the original fields, setting new consolidated fields to defaults
    #[allow(clippy::too_many_arguments)]
    pub fn with_defaults(
        id: Uuid,
        season_id: Uuid,
        home_team_id: Uuid,
        away_team_id: Uuid,
        week_number: i32,
        is_first_leg: bool,
        status: GameStatus,
        winner_team_id: Option<Uuid>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            season_id,
            home_team_id,
            away_team_id,
            week_number,
            is_first_leg,
            status,
            winner_team_id,
            created_at,
            updated_at,
            // Default values for new consolidated fields
            home_score: 0,
            away_score: 0,
            game_start_time: None,
            game_end_time: None,
            last_score_time: None,
            last_scorer_id: None,
            last_scorer_name: None,
            last_scorer_team: None,
        }
    }
    
    /// Create a new LeagueGame with basic fields, defaulting new consolidated fields to None
    #[allow(clippy::too_many_arguments)]
    pub fn new_basic(
        id: Uuid,
        season_id: Uuid,
        home_team_id: Uuid,
        away_team_id: Uuid,
        week_number: i32,
        is_first_leg: bool,
        status: GameStatus,
        home_score: i32,
        away_score: i32,
        winner_team_id: Option<Uuid>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            season_id,
            home_team_id,
            away_team_id,
            week_number,
            is_first_leg,
            status,
            winner_team_id,
            created_at,
            updated_at,
            // Default new consolidated fields
            home_score,
            away_score,
            game_start_time: None,
            game_end_time: None,
            last_score_time: None,
            last_scorer_id: None,
            last_scorer_name: None,
            last_scorer_team: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum GameStatus {
    Scheduled,
    InProgress,
    Finished,
    Evaluated,
    Postponed,
}

impl From<String> for GameStatus {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "in_progress" | "in-progress" => GameStatus::InProgress,
            "finished" => GameStatus::Finished,
            "evaluated" => GameStatus::Evaluated,
            "postponed" => GameStatus::Postponed,
            _ => GameStatus::Scheduled,
        }
    }
}

#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct LeagueStanding {
    pub id: Uuid,
    pub season_id: Uuid,
    pub team_id: Uuid,
    pub games_played: i32,
    pub wins: i32,
    pub draws: i32,
    pub losses: i32,
    // Points is a generated column in the database, so it can be NULL in some edge cases
    pub points: Option<i32>,
    pub position: i32,
    pub last_updated: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_points_scored: Option<i32>,
}

// Request/Response DTOs
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateSeasonRequest {
    pub league_id: Uuid,
    pub name: String,
    pub start_date: DateTime<Utc>,
    pub team_ids: Vec<Uuid>,
    pub game_duration_seconds: Option<i64>, // Optional, defaults to 518400 seconds (6 days) if not provided
    pub games_per_matchup: Option<i32>, // Optional, defaults to 2 (double round-robin) if not provided
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LeagueScheduleResponse {
    pub season: LeagueSeason,
    pub games: Vec<GameWithTeams>,
    pub next_game_time: DateTime<Utc>,
    pub total_weeks: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameWithTeams {
    pub game: LeagueGame,
    pub home_team_name: String,
    pub away_team_name: String,
    pub home_team_color: String,
    pub away_team_color: String,
    pub home_team_power: Option<f32>,
    pub away_team_power: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LeagueStandingsResponse {
    pub season: LeagueSeason,
    pub standings: Vec<StandingWithTeam>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StandingWithTeam {
    pub standing: LeagueStanding,
    pub team_name: String,
    pub team_color: String,
    pub recent_form: Vec<char>, // W, D, L for last 5 games
    pub team_power: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NextGameInfo {
    pub next_game: Option<GameWithTeams>,
    pub countdown_seconds: Option<i64>,
    pub week_number: Option<i32>,
    pub games_this_week: Vec<GameWithTeams>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameWeekResponse {
    pub week_number: i32,
    pub game_time: DateTime<Utc>,
    pub games: Vec<GameWithTeams>,
    pub is_current_week: bool,
    pub countdown_seconds: Option<i64>, // Only for current week
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameResultRequest {
    pub home_score: i32,
    pub away_score: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CountdownQuery {
    pub season_id: Option<Uuid>,
}

impl fmt::Display for CountdownQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "season_id: {:?}", self.season_id)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpcomingGamesQuery {
    pub season_id: Option<Uuid>,
    pub limit: Option<i64>,
}

impl fmt::Display for UpcomingGamesQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "season_id: {:?}, limit: {:?}", self.season_id, self.limit)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecentResultsQuery {
    pub season_id: Option<Uuid>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

// Helper implementations
impl GameStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            GameStatus::Scheduled => "scheduled",
            GameStatus::InProgress => "in_progress",
            GameStatus::Finished => "finished",
            GameStatus::Evaluated => "evaluated",
            GameStatus::Postponed => "postponed",
        }
    }
}

impl LeagueStanding {
    /// Get points with a safe default of 0 if None
    pub fn get_points(&self) -> i32 {
        self.points.unwrap_or(0)
    }

    /// Calculate form percentage based on points
    pub fn form_percentage(&self) -> f32 {
        if self.games_played == 0 {
            return 0.0;
        }
        (self.get_points() as f32) / (self.games_played as f32 * 3.0) * 100.0
    }

    /// Calculate points manually (in case the generated column isn't working)
    pub fn calculate_points(&self) -> i32 {
        self.wins * 3 + self.draws
    }

    /// Ensure points are correctly calculated
    pub fn ensure_points_calculated(&mut self) {
        if self.points.is_none() {
            self.points = Some(self.calculate_points());
        }
    }
}

// Game Summary models
#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct GameSummary {
    pub id: Uuid,
    pub game_id: Uuid,

    // Game Overview
    pub final_home_score: i32,
    pub final_away_score: i32,
    pub game_start_date: DateTime<Utc>,
    pub game_end_date: DateTime<Utc>,
    pub mvp_user_id: Option<Uuid>,
    pub mvp_username: Option<String>,
    pub mvp_team_id: Option<Uuid>,
    pub mvp_score_contribution: Option<i32>,
    pub mvp_profile_picture_url: Option<String>,
    pub lvp_user_id: Option<Uuid>,
    pub lvp_username: Option<String>,
    pub lvp_team_id: Option<Uuid>,
    pub lvp_score_contribution: Option<i32>,
    pub lvp_profile_picture_url: Option<String>,

    // Home Team Statistics
    pub home_team_avg_score_per_player: Option<f32>,
    pub home_team_total_workouts: i32,
    pub home_team_top_scorer_id: Option<Uuid>,
    pub home_team_top_scorer_username: Option<String>,
    pub home_team_top_scorer_points: Option<i32>,
    pub home_team_lowest_performer_id: Option<Uuid>,
    pub home_team_lowest_performer_username: Option<String>,
    pub home_team_lowest_performer_points: Option<i32>,

    // Away Team Statistics
    pub away_team_avg_score_per_player: Option<f32>,
    pub away_team_total_workouts: i32,
    pub away_team_top_scorer_id: Option<Uuid>,
    pub away_team_top_scorer_username: Option<String>,
    pub away_team_top_scorer_points: Option<i32>,
    pub away_team_lowest_performer_id: Option<Uuid>,
    pub away_team_lowest_performer_username: Option<String>,
    pub away_team_lowest_performer_points: Option<i32>,

    // Metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameSummaryResponse {
    pub summary: GameSummary,
    pub home_team_name: String,
    pub away_team_name: String,
}