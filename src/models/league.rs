// src/models/league.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct LeagueSeason {
    pub id: Uuid,
    pub name: String,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct LeagueGame {
    pub id: Uuid,
    pub season_id: Uuid,
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
    pub scheduled_time: DateTime<Utc>,
    pub week_number: i32,
    pub is_first_leg: bool,
    pub status: GameStatus,
    pub home_score: Option<i32>,
    pub away_score: Option<i32>,
    pub winner_team_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
pub enum GameStatus {
    Scheduled,
    Live,
    Finished,
    Postponed,
}

impl From<String> for GameStatus {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "live" => GameStatus::Live,
            "finished" => GameStatus::Finished,
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
    pub points: i32,
    pub position: i32,
    pub last_updated: DateTime<Utc>,
}

// Request/Response DTOs
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateSeasonRequest {
    pub name: String,
    pub start_date: DateTime<Utc>,
    pub team_ids: Vec<Uuid>,
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
    pub team_icon: String,
    pub recent_form: Vec<char>, // W, D, L for last 5 games
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NextGameInfo {
    pub next_game: Option<GameWithTeams>,
    pub countdown_seconds: i64,
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

// Helper implementations
impl GameStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            GameStatus::Scheduled => "scheduled",
            GameStatus::Live => "live",
            GameStatus::Finished => "finished",
            GameStatus::Postponed => "postponed",
        }
    }
}

impl LeagueStanding {
    pub fn form_percentage(&self) -> f32 {
        if self.games_played == 0 {
            return 0.0;
        }
        (self.points as f32) / (self.games_played as f32 * 3.0) * 100.0
    }
}