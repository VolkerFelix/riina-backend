use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::models::common::{MatchResult, PlayerStats, TeamStandings};

/// Game-specific WebSocket message types
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "event_type")]
pub enum GameEvent {
    #[serde(rename = "player_joined")]
    PlayerJoined {
        user_id: Uuid,
        username: String,
        position: Position,
        timestamp: DateTime<Utc>,
    },

    #[serde(rename = "player_left")]
    PlayerLeft {
        user_id: Uuid,
        username: String,
        timestamp: DateTime<Utc>,
    },

    #[serde(rename = "avatar_updated")]
    AvatarUpdated {
        user_id: Uuid,
        username: String,
        stats: PlayerStats,
        position: Position,
        timestamp: DateTime<Utc>,
    },

    #[serde(rename = "leaderboard_update")]
    LeaderboardUpdate {
        daily_rankings: Vec<PlayerRanking>,
        updated_at: DateTime<Utc>,
    },

    #[serde(rename = "battle_started")]
    BattleStarted {
        battle_id: Uuid,
        team_a: BattleTeam,
        team_b: BattleTeam,
        start_time: DateTime<Utc>,
    },

    #[serde(rename = "battle_ended")]
    BattleEnded {
        battle_id: Uuid,
        winner_team_id: Uuid,
        results: BattleResults,
        end_time: DateTime<Utc>,
    },

    #[serde(rename = "territory_conquered")]
    TerritoryConquered {
        territory_id: Uuid,
        territory_name: String,
        conquering_team_id: Uuid,
        conquering_team_name: String,
        conquered_at: DateTime<Utc>,
    },

    #[serde(rename = "health_data_processed")]
    HealthDataProcessed {
        user_id: Uuid,
        sync_id: Uuid,
        stat_changes: StatChanges,
        timestamp: DateTime<Utc>,
    },

    #[serde(rename = "team_invitation")]
    TeamInvitation {
        invitation_id: Uuid,
        from_user_id: Uuid,
        from_username: String,
        team_name: String,
        message: Option<String>,
        expires_at: DateTime<Utc>,
    },

    #[serde(rename = "notification")]
    Notification {
        notification_id: Uuid,
        user_id: Uuid,
        title: String,
        message: String,
        notification_type: NotificationType,
        action_url: Option<String>,
        created_at: DateTime<Utc>,
    },

    #[serde(rename = "games_evaluated")]
    GamesEvaluated {
        evaluation_id: Uuid,
        date: String, // ISO date string
        total_games: usize,
        game_results: Vec<GameResult>,
        standings_updated: bool,
        evaluated_at: DateTime<Utc>,
    },

    #[serde(rename = "team_standings_updated")]
    TeamStandingsUpdated {
        league_id: Uuid,
        league_name: String,
        standings: Vec<TeamStandings>,
        updated_at: DateTime<Utc>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

// Using PlayerStats from common module instead of duplicate AvatarStats

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlayerRanking {
    pub user_id: Uuid,
    pub username: String,
    pub total_stats: u32,
    pub rank: u32,
    pub score: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BattleTeam {
    pub team_id: Uuid,
    pub team_name: String,
    pub members: Vec<BattleMember>,
    pub strategy: BattleStrategy,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BattleMember {
    pub user_id: Uuid,
    pub username: String,
    pub stats: PlayerStats,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum BattleStrategy {
    Attack,
    Defend,
    Hold,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BattleResults {
    pub winner_score: u32,
    pub loser_score: u32,
    pub mvp_user_id: Uuid,
    pub stat_contributions: Vec<StatContribution>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatContribution {
    pub user_id: Uuid,
    pub username: String,
    pub stamina_contribution: u32,
    pub strength_contribution: u32,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatChanges {
    pub stamina_change: i32,
    pub strength_change: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum NotificationType {
    BattleInvite,
    TeamInvite,
    Achievement,
    DailyChallenge,
    TerritoryAlert,
    System,
    GameResult,
    StandingsUpdate,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GameResult {
    pub game_id: Uuid,
    pub home_team_id: Uuid,
    pub home_team_name: String,
    pub away_team_id: Uuid,
    pub away_team_name: String,
    pub home_score: u32,
    pub away_score: u32,
    pub winner_team_id: Option<Uuid>,
    pub match_result: MatchResult,
}


// Using TeamStandings from common module instead of duplicate TeamStanding