use uuid::Uuid;
use sqlx::PgPool;
use crate::models::common::MatchResult;

#[derive(Debug, Clone)]
pub struct GameStats {
    pub game_id: Uuid,
    pub home_team_name: String,
    pub away_team_name: String,
    pub home_team_score: u32,
    pub away_team_score: u32,
    pub home_team_result: MatchResult,
    pub away_team_result: MatchResult,
    pub winner_team_id: Option<Uuid>,
    pub home_score: u32,
    pub away_score: u32,
}