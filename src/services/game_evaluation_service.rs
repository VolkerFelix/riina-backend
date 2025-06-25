use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use redis::AsyncCommands;

use crate::game::game_evaluator::{GameEvaluator, GameStats};
use crate::models::game_events::{GameEvent, GameResult, NotificationType};
use crate::models::common::MatchResult;

pub struct GameEvaluationService {
    pool: PgPool,
    redis_client: Option<Arc<redis::Client>>,
}

#[derive(Debug)]
pub struct EvaluationResult {
    pub games_evaluated: usize,
    pub games_updated: usize,
    pub errors: Vec<String>,
    pub game_results: HashMap<Uuid, GameStats>,
}

impl GameEvaluationService {
    pub fn new(pool: PgPool) -> Self {
        Self { 
            pool,
            redis_client: None,
        }
    }

    pub fn new_with_redis(pool: PgPool, redis_client: Option<Arc<redis::Client>>) -> Self {
        Self { 
            pool,
            redis_client,
        }
    }

    /// Evaluate all scheduled games and update their results in the database
    pub async fn evaluate_and_update_todays_games(&self) -> Result<EvaluationResult, sqlx::Error> {
        tracing::info!("🎯 Starting game evaluation process");
        
        let mut result = EvaluationResult {
            games_evaluated: 0,
            games_updated: 0,
            errors: Vec::new(),
            game_results: HashMap::new(),
        };

        // Get all game results
        let game_evaluations = match GameEvaluator::evaluate_todays_games(&self.pool).await {
            Ok(evaluations) => evaluations,
            Err(e) => {
                let error_msg = format!("Failed to evaluate today's games: {}", e);
                tracing::error!("{}", error_msg);
                result.errors.push(error_msg);
                return Ok(result);
            }
        };

        result.games_evaluated = game_evaluations.len();
        tracing::info!("📊 {} games evaluated", result.games_evaluated);

        // Save the game results
        for (game_id, game_stats) in game_evaluations {
            tracing::info!("🎮 Saving game {} result: {} - {}", 
                game_id, game_stats.home_team_score, game_stats.away_team_score);

            // Update the game result in database
            match self.update_game_result(game_id, &game_stats).await {
                Ok(_) => {
                    result.games_updated += 1;
                    tracing::info!("✅ Successfully updated game {} result", game_id);
                    result.game_results.insert(game_id, game_stats.clone());
                }
                Err(e) => {
                    let error_msg = format!("Failed to save game {} in database: {}", game_id, e);
                    tracing::error!("{}", error_msg);
                    result.errors.push(error_msg);
                }
            }
        }

        tracing::info!("🏁 Game evaluation completed: {}/{} games stored successfully", 
            result.games_updated, result.games_evaluated);

        if !result.errors.is_empty() {
            tracing::warn!("⚠️  {} errors occurred during evaluation", result.errors.len());
            for error in &result.errors {
                tracing::warn!("   - {}", error);
            }
        }

        // Send WebSocket notifications for successful evaluations
        if result.games_updated > 0 {
            if let Err(e) = self.broadcast_game_evaluation_results(&result.game_results, chrono::Utc::now().date_naive()).await {
                tracing::error!("Failed to broadcast game evaluation results: {}", e);
                // Don't fail the entire operation for notification failures
            }
        }

        Ok(result)
    }

    /// Update a specific game's result in the database
    async fn update_game_result(&self, game_id: Uuid, game_stats: &GameStats) -> Result<(), sqlx::Error> {
        // Start a transaction to ensure atomic updates
        let mut tx = self.pool.begin().await?;

        // Update the game result
        sqlx::query!(
            r#"
            UPDATE league_games 
            SET 
                home_score = $2,
                away_score = $3,
                winner_team_id = $4,
                status = 'finished',
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

        // Commit the transaction
        tx.commit().await?;

        Ok(())
    }

    /// Evaluate a specific game (for testing or manual evaluation)
    pub async fn evaluate_specific_game(&self, game_id: Uuid) -> Result<GameStats, sqlx::Error> {
        tracing::info!("🎯 Evaluating specific game: {}", game_id);

        // Get the game details
        let game = sqlx::query!(
            r#"
            SELECT home_team_id, away_team_id, status
            FROM league_games 
            WHERE id = $1
            "#,
            game_id
        )
        .fetch_optional(&self.pool)
        .await?;

        match game {
            Some(game_data) => {
                if game_data.status != "scheduled" {
                    return Err(sqlx::Error::RowNotFound);
                }

                let game_stats = GameEvaluator::evaluate_game(
                    &self.pool, 
                    &game_data.home_team_id, 
                    &game_data.away_team_id
                ).await?;

                tracing::info!("✅ Game {} evaluated: {} - {}", 
                    game_id, game_stats.home_team_score, game_stats.away_team_score);

                Ok(game_stats)
            }
            None => {
                tracing::error!("❌ Game {} not found", game_id);
                Err(sqlx::Error::RowNotFound)
            }
        }
    }

    /// Get summary of today's scheduled games
    pub async fn get_todays_game_summary(&self) -> Result<GameSummary, sqlx::Error> {
        let summary = sqlx::query!(
            r#"
            SELECT 
                COUNT(*) as total_games,
                COUNT(CASE WHEN status = 'scheduled' THEN 1 END) as scheduled_games,
                COUNT(CASE WHEN status = 'finished' THEN 1 END) as finished_games,
                COUNT(CASE WHEN status = 'postponed' THEN 1 END) as postponed_games
            FROM league_games 
            WHERE DATE(scheduled_time) = CURRENT_DATE
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(GameSummary {
            total_games: summary.total_games.unwrap_or(0) as usize,
            scheduled_games: summary.scheduled_games.unwrap_or(0) as usize,
            finished_games: summary.finished_games.unwrap_or(0) as usize,
            postponed_games: summary.postponed_games.unwrap_or(0) as usize,
        })
    }

    pub async fn get_games_summary_for_date(&self, date: chrono::NaiveDate) -> Result<GameSummary, sqlx::Error> {
        let summary = sqlx::query!(
            r#"
            SELECT 
                COUNT(*) as total_games,
                COUNT(CASE WHEN status = 'scheduled' THEN 1 END) as scheduled_games,
                COUNT(CASE WHEN status = 'finished' THEN 1 END) as finished_games,
                COUNT(CASE WHEN status = 'postponed' THEN 1 END) as postponed_games
            FROM league_games 
            WHERE DATE(scheduled_time) = $1
            "#,
            date
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(GameSummary {
            total_games: summary.total_games.unwrap_or(0) as usize,
            scheduled_games: summary.scheduled_games.unwrap_or(0) as usize,
            finished_games: summary.finished_games.unwrap_or(0) as usize,
            postponed_games: summary.postponed_games.unwrap_or(0) as usize,
        })
    }

    pub async fn evaluate_and_update_games_for_date(&self, date: chrono::NaiveDate) -> Result<EvaluationResult, sqlx::Error> {
        tracing::info!("🎯 Starting game evaluation process for date: {}", date);
        
        let mut result = EvaluationResult {
            games_evaluated: 0,
            games_updated: 0,
            errors: Vec::new(),
            game_results: HashMap::new(),
        };

        // Get all game results for the specific date
        let game_evaluations = match GameEvaluator::evaluate_games_for_date(&self.pool, date).await {
            Ok(evaluations) => evaluations,
            Err(e) => {
                let error_msg = format!("Failed to evaluate games for date {}: {}", date, e);
                tracing::error!("{}", error_msg);
                result.errors.push(error_msg);
                return Ok(result);
            }
        };

        if game_evaluations.is_empty() {
            tracing::info!("No scheduled games found for date: {}", date);
            return Ok(result);
        }

        tracing::info!("📊 Found {} scheduled games for evaluation on {}", game_evaluations.len(), date);

        // Process each game evaluation
        for game_eval in game_evaluations {
            result.games_evaluated += 1;
            
            match self.update_game_result(game_eval.game_id, &game_eval).await {
                Ok(_) => {
                    result.games_updated += 1;
                    result.game_results.insert(game_eval.game_id, game_eval.clone());
                    tracing::info!("✅ Updated game {} result: {} vs {} = {}-{}", 
                        game_eval.game_id, 
                        game_eval.home_team_name, 
                        game_eval.away_team_name,
                        game_eval.home_score, 
                        game_eval.away_score
                    );
                }
                Err(e) => {
                    let error_msg = format!("Failed to update game {}: {}", game_eval.game_id, e);
                    tracing::error!("{}", error_msg);
                    result.errors.push(error_msg);
                }
            }
        }

        tracing::info!("🏁 Game evaluation completed: {}/{} games updated successfully", 
            result.games_updated, result.games_evaluated);

        // Send WebSocket notifications for successful evaluations
        if result.games_updated > 0 {
            if let Err(e) = self.broadcast_game_evaluation_results(&result.game_results, date).await {
                tracing::error!("Failed to broadcast game evaluation results: {}", e);
                // Don't fail the entire operation for notification failures
            }
        }

        Ok(result)
    }

    /// Broadcast game evaluation results to all league participants via WebSocket
    async fn broadcast_game_evaluation_results(
        &self,
        game_results: &HashMap<Uuid, GameStats>,
        date: chrono::NaiveDate,
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
        if let Some(redis_client) = &self.redis_client {
            let mut conn = redis_client.get_async_connection().await?;
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
        } else {
            tracing::warn!("⚠️  No Redis client available for broadcasting game evaluation results");
        }
        Ok(())
    }

    /// Send individual notifications to team members about their game results
    async fn send_team_notifications_with_game_info(&self, game_results: &HashMap<Uuid, GameStats>) -> Result<(), Box<dyn std::error::Error>> {
        for (game_id, stats) in game_results {
            // Get team IDs from the database for this game
            if let Ok(game_info) = self.get_game_team_info(*game_id).await {
                // Get all team members for both teams
                let team_members = self.get_team_members_for_game(game_info.home_team_id, game_info.away_team_id).await?;
                
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
                        title: format!("Match Result: {}", result_text),
                        message: format!(
                            "Your team scored {} against {} ({}). Final score: {} - {}",
                            user_team_score, opponent_name, opponent_score, user_team_score, opponent_score
                        ),
                        notification_type: NotificationType::GameResult,
                        action_url: Some(format!("/game/{}", game_id)),
                        created_at: Utc::now(),
                    };

                    self.send_user_notification(&member.user_id, &notification).await?;
                }
            }
        }
        Ok(())
    }

    /// Send notification to a specific user using existing Redis pattern
    async fn send_user_notification(&self, user_id: &Uuid, notification: &GameEvent) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(redis_client) = &self.redis_client {
            let mut conn = redis_client.get_async_connection().await?;
            let message = serde_json::to_string(notification)?;
            let user_channel = format!("game:events:user:{}", user_id);
            
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
        }
        Ok(())
    }

    /// Get game team information
    async fn get_game_team_info(&self, game_id: Uuid) -> Result<GameTeamInfo, sqlx::Error> {
        let game_info = sqlx::query!(
            r#"
            SELECT home_team_id, away_team_id
            FROM league_games
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
            username: row.username,
            team_name: row.team_name,
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
    pub username: String,
    pub team_name: String,
}

#[derive(Debug)]
struct GameTeamInfo {
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
}