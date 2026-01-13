use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use redis::AsyncCommands;

use crate::models::game_events::{GameEvent, GameResult, NotificationType};
use crate::models::common::MatchResult;
use crate::league::standings::StandingsService;
use crate::models::league::{LeagueGame, GameStatus};
use crate::game::game_evaluator::GameStats;
use crate::services::game_summary_service::GameSummaryService;

#[derive(Debug)]
pub struct GameEvaluationService {
    pool: PgPool,
    redis_client: Arc<redis::Client>,
    standings: StandingsService,
    summary_service: GameSummaryService,
}

#[derive(Debug)]
pub struct EvaluationResult {
    pub games_evaluated: usize,
    pub games_updated: usize,
    pub errors: Vec<String>,
    pub game_results: HashMap<Uuid, GameStats>,
}

impl GameEvaluationService {
    pub fn new(pool: PgPool, redis_client: Arc<redis::Client>) -> Self {
        Self {
            standings: StandingsService::new(pool.clone()),
            summary_service: GameSummaryService::new(pool.clone()),
            pool,
            redis_client,
        }
    }

    /// Update a specific game's result in the database and update standings
    async fn update_game_result(&self, game_id: Uuid, game_stats: &GameStats) -> Result<(), sqlx::Error> {
        // First, get the game details before updating
        let game_record = sqlx::query_as!(
            LeagueGame,
            r#"
            SELECT 
                id, season_id, home_team_id, away_team_id, 
                week_number, is_first_leg, status as "status: GameStatus", 
winner_team_id,
                created_at, updated_at,
                home_score, away_score, game_start_time, game_end_time,
                last_score_time, last_scorer_id, last_scorer_name, last_scorer_team
            FROM games 
            WHERE id = $1
            "#,
            game_id
        )
        .fetch_one(&self.pool)
        .await?;

        // Start a transaction to ensure atomic updates
        let mut tx = self.pool.begin().await?;

        // Update the game result and mark as evaluated
        sqlx::query!(
            r#"
            UPDATE games 
            SET 
                home_score = $2,
                away_score = $3,
                winner_team_id = $4,
                status = 'evaluated',
                updated_at = $5
            WHERE id = $1
            "#,
            game_id,
            game_stats.home_team_score as i32,
            game_stats.away_team_score as i32,
            game_stats.winner_team_id,
            Utc::now()
        )
        .execute(&mut *tx)
        .await?;

        // Commit the game update transaction
        tx.commit().await?;

        // Update standings with the updated game record
        let mut updated_game = game_record;
        updated_game.home_score = game_stats.home_team_score as i32;
        updated_game.away_score = game_stats.away_team_score as i32;
        updated_game.winner_team_id = game_stats.winner_team_id;
        updated_game.status = GameStatus::Evaluated;

        // Update standings
        match self.standings.update_after_game_result(
            &updated_game,
            game_stats.home_team_score as i32,
            game_stats.away_team_score as i32
        ).await {
            Ok(_) => {
                tracing::info!("✅ Successfully updated game {} and standings: {} - {}",
                    game_id, game_stats.home_team_score, game_stats.away_team_score);
            }
            Err(e) => {
                tracing::error!("❌ Failed to update standings for game {}: {}", game_id, e);
                return Err(e);
            }
        }

        // Create game summary
        match self.summary_service.create_game_summary(&updated_game).await {
            Ok(summary) => {
                tracing::info!("✅ Successfully created game summary for game {}", game_id);
                tracing::debug!("Game summary details: MVP={:?}, LVP={:?}",
                    summary.mvp_username, summary.lvp_username);

                // Broadcast game summary created event
                let summary_event = GameEvent::GameSummaryCreated {
                    game_id,
                    summary_id: summary.id,
                    home_team_id: updated_game.home_team_id,
                    away_team_id: updated_game.away_team_id,
                    mvp_user_id: summary.mvp_user_id,
                    mvp_username: summary.mvp_username.clone(),
                    lvp_user_id: summary.lvp_user_id,
                    lvp_username: summary.lvp_username.clone(),
                    final_home_score: summary.final_home_score,
                    final_away_score: summary.final_away_score,
                    created_at: Utc::now(),
                };

                if let Err(e) = self.broadcast_to_global_channel(&summary_event).await {
                    tracing::error!("Failed to broadcast game summary event: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("❌ Failed to create game summary for game {}: {}", game_id, e);
                // Don't fail the entire evaluation if summary creation fails
                // The game is still evaluated and standings are updated
            }
        }

        Ok(())
    }

    /// Evaluate and update finished live games
    pub async fn evaluate_finished_live_games(&self, game_ids: &Vec<Uuid>) -> Result<Vec<GameStats>, sqlx::Error> {
        if game_ids.is_empty() {
            tracing::info!("🎯 [EVALUATOR] No games to evaluate");
            return Ok(Vec::new());
        }
        tracing::info!("🎯 [EVALUATOR] Starting evaluation of {} finished live games: {:?}", game_ids.len(), game_ids);

        // Get the game details
        tracing::info!("🔍 [EVALUATOR] Fetching game data from database for {} games", game_ids.len());
        let games = sqlx::query!(
            r#"
            SELECT id, home_team_id, away_team_id, home_score, away_score
            FROM games
            WHERE id = ANY($1) and status = 'finished'
            "#,
            game_ids
        )
        .fetch_all(&self.pool)
        .await?;

        tracing::info!("🔍 [EVALUATOR] Found {} games with status='finished' to evaluate", games.len());
        if games.len() != game_ids.len() {
            tracing::warn!("⚠️  [EVALUATOR] Expected {} games but found only {} with status='finished'",
                game_ids.len(), games.len());
        }

        let mut results = Vec::new();

        for game_data in games {
            let game_id = game_data.id;

            // Use the scores from the games table directly (already consolidated)
            let game_stats = {
                let home_score = game_data.home_score;
                let away_score = game_data.away_score;
                let winner_team_id = if home_score > away_score {
                    Some(game_data.home_team_id)
                } else if away_score > home_score {
                    Some(game_data.away_team_id)
                } else {
                    None
                };

                GameStats {
                    game_id,
                    home_team_name: String::new(),
                    away_team_name: String::new(),
                    home_team_score: home_score as u32,
                    away_team_score: away_score as u32,
                    home_team_result: if home_score > away_score { 
                        MatchResult::Win 
                    } else if home_score < away_score { 
                        MatchResult::Loss 
                    } else { 
                        MatchResult::Draw 
                    },
                    away_team_result: if away_score > home_score { 
                        MatchResult::Win 
                    } else if away_score < home_score { 
                        MatchResult::Loss 
                    } else { 
                        MatchResult::Draw 
                    },
                    winner_team_id,
                    home_score: home_score as u32,
                    away_score: away_score as u32,
                }
            };

            // Update the game result in the database
            tracing::info!("🎯 [EVALUATOR] Processing finished game {}: home={} - away={}",
                game_id, game_stats.home_team_score, game_stats.away_team_score);

            match self.update_game_result(game_id, &game_stats).await {
                Ok(_) => {
                    tracing::info!("✅ [EVALUATOR] Game {} evaluated and updated: {} - {}",
                        game_id, game_stats.home_team_score, game_stats.away_team_score);
                    results.push(game_stats);
                }
                Err(e) => {
                    tracing::error!("❌ [EVALUATOR] Failed to update game {}: {}", game_id, e);
                }
            }
        }

        tracing::info!("✅ [EVALUATOR] Completed evaluation of {} games", results.len());

        // Send WebSocket notifications if we have results
        if !results.is_empty() {
            tracing::info!("📡 [EVALUATOR] Broadcasting results for {} evaluated games", results.len());
            let game_results_map: HashMap<Uuid, GameStats> = results.iter()
                .map(|stats| (stats.game_id, stats.clone()))
                .collect();
            
            if let Err(e) = self.broadcast_game_evaluation_results(&game_results_map, Utc::now()).await {
                tracing::error!("Failed to broadcast game evaluation results: {}", e);
                // Don't fail the entire operation for notification failures
            }
        }

        Ok(results)
    }

    /// Broadcast game evaluation results to all league participants via WebSocket
    async fn broadcast_game_evaluation_results(
        &self,
        game_results: &HashMap<Uuid, GameStats>,
        date: DateTime<Utc>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("📡 Broadcasting game evaluation results for {} games on {}", 
            game_results.len(), date);

        // Get game details to include team IDs
        let mut ws_game_results = Vec::new();
        for (game_id, stats) in game_results {
            // Get team IDs from the database for this game
            if let Ok(game_info) = self.get_game_team_info(*game_id).await {
                let match_result = if stats.winner_team_id.is_some() {
                    if stats.home_score > stats.away_score {
                        MatchResult::Win
                    } else {
                        MatchResult::Loss
                    }
                } else {
                    MatchResult::Draw
                };

                let game_result = GameResult {
                    game_id: *game_id,
                    home_team_id: game_info.home_team_id,
                    home_team_name: stats.home_team_name.clone(),
                    away_team_id: game_info.away_team_id,
                    away_team_name: stats.away_team_name.clone(),
                    home_score: stats.home_score,
                    away_score: stats.away_score,
                    winner_team_id: stats.winner_team_id,
                    match_result,
                };
                ws_game_results.push(game_result);
            }
        }

        // Create the main games evaluated event
        let games_evaluated_event = GameEvent::GamesEvaluated {
            evaluation_id: Uuid::new_v4(),
            date: date.to_string(),
            total_games: game_results.len(),
            game_results: ws_game_results,
            standings_updated: true,
            evaluated_at: Utc::now(),
        };

        // Broadcast to global channel using existing pattern
        self.broadcast_to_global_channel(&games_evaluated_event).await?;

        // Send individual notifications to affected team members
        self.send_team_notifications_with_game_info(game_results).await?;

        tracing::info!("✅ Successfully broadcasted game evaluation results");
        Ok(())
    }

    /// Broadcast event to global game events channel using existing Redis pattern
    async fn broadcast_to_global_channel(&self, event: &GameEvent) -> Result<(), Box<dyn std::error::Error>> {
            let mut conn = self.redis_client.get_async_connection().await?;
            let message = serde_json::to_string(event)?;
            
            let global_channel = "game:events:global";
            let result: Result<i32, redis::RedisError> = conn.publish(global_channel, message).await;
            
            match result {
                Ok(receivers) => {
                    tracing::info!("📤 Published game evaluation event to {} global subscribers", receivers);
                }
                Err(e) => {
                    tracing::error!("❌ Failed to publish game evaluation event: {}", e);
                    return Err(Box::new(e));
                }
            }
        Ok(())
    }

    /// Send individual notifications to team members about their game results
    async fn send_team_notifications_with_game_info(&self, game_results: &HashMap<Uuid, GameStats>) -> Result<(), Box<dyn std::error::Error>> {
        tracing::debug!("📨 Sending team notifications for {} games", game_results.len());
        for (game_id, stats) in game_results {
            // Get team IDs from the database for this game
            if let Ok(game_info) = self.get_game_team_info(*game_id).await {
                tracing::debug!("📨 Processing game {} with teams {} vs {}", game_id, game_info.home_team_id, game_info.away_team_id);
                // Get all team members for both teams
                let team_members = self.get_team_members_for_game(game_info.home_team_id, game_info.away_team_id).await?;
                tracing::debug!("📨 Found {} team members for game {}", team_members.len(), game_id);
                
                for member in team_members {
                    let is_home_team = member.team_id == game_info.home_team_id;
                    let user_team_score = if is_home_team { stats.home_score } else { stats.away_score };
                    let opponent_score = if is_home_team { stats.away_score } else { stats.home_score };
                    let opponent_name = if is_home_team { &stats.away_team_name } else { &stats.home_team_name };
                    
                    let result_text = match stats.winner_team_id {
                        Some(winner_id) if winner_id == member.team_id => "Victory! 🏆",
                        Some(_) => "Defeat 😔",
                        None => "Draw ⚖️",
                    };

                    let notification = GameEvent::Notification {
                        notification_id: Uuid::new_v4(),
                        user_id: member.user_id,
                        title: format!("Match Result: {result_text}"),
                        message: format!(
                            "Your team scored {user_team_score} against {opponent_name} ({opponent_score}). Final score: {user_team_score} - {opponent_score}"
                        ),
                        notification_type: NotificationType::GameResult,
                        action_url: Some(format!("/game/{game_id}")),
                        created_at: Utc::now(),
                    };

                    tracing::debug!("📨 Sending notification to user {} for game {}", member.user_id, game_id);
                    self.send_user_notification(&member.user_id, &notification).await?;
                }
            }
        }
        Ok(())
    }

    /// Send notification to a specific user using existing Redis pattern
    async fn send_user_notification(&self, user_id: &Uuid, notification: &GameEvent) -> Result<(), Box<dyn std::error::Error>> {
            let mut conn = self.redis_client.get_async_connection().await?;
            let message = serde_json::to_string(notification)?;
            let user_channel = format!("game:events:user:{user_id}");
            
            let result: Result<i32, redis::RedisError> = conn.publish(&user_channel, message).await;
            
            match result {
                Ok(receivers) => {
                    tracing::debug!("📤 Sent game result notification to user {} ({} subscribers)", user_id, receivers);
                }
                Err(e) => {
                    tracing::error!("❌ Failed to send notification to user {}: {}", user_id, e);
                    return Err(Box::new(e));
                }
            }
        Ok(())
    }

    /// Get game team information
    async fn get_game_team_info(&self, game_id: Uuid) -> Result<GameTeamInfo, sqlx::Error> {
        let game_info = sqlx::query!(
            r#"
            SELECT home_team_id, away_team_id
            FROM games
            WHERE id = $1
            "#,
            game_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(GameTeamInfo {
            home_team_id: game_info.home_team_id,
            away_team_id: game_info.away_team_id,
        })
    }

    /// Get team members for both teams involved in a game
    async fn get_team_members_for_game(&self, home_team_id: Uuid, away_team_id: Uuid) -> Result<Vec<TeamMember>, sqlx::Error> {
        let members = sqlx::query!(
            r#"
            SELECT tm.user_id, tm.team_id, u.username, t.team_name
            FROM team_members tm
            JOIN users u ON tm.user_id = u.id
            JOIN teams t ON tm.team_id = t.id
            WHERE tm.team_id IN ($1, $2) AND tm.status = 'active'
            "#,
            home_team_id,
            away_team_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(members.into_iter().map(|row| TeamMember {
            user_id: row.user_id,
            team_id: row.team_id,
        }).collect())
    }
}

#[derive(Debug)]
pub struct GameSummary {
    pub total_games: usize,
    pub scheduled_games: usize,
    pub finished_games: usize,
    pub postponed_games: usize,
}

impl std::fmt::Display for GameSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Today's Games: {} total ({} scheduled, {} finished, {} postponed)", 
            self.total_games, self.scheduled_games, self.finished_games, self.postponed_games)
    }
}

impl std::fmt::Display for EvaluationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Evaluation Result: {}/{} games updated successfully, {} errors", 
            self.games_updated, self.games_evaluated, self.errors.len())
    }
}

#[derive(Debug)]
struct TeamMember {
    pub user_id: Uuid,
    pub team_id: Uuid,
}

#[derive(Debug)]
struct GameTeamInfo {
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
}