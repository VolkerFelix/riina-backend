use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use redis::AsyncCommands;

use crate::models::game_events::{GameEvent, GameResult, NotificationType};
use crate::models::common::MatchResult;
use crate::league::standings::StandingsService;
use crate::models::league::{LeagueGame, GameStatus};
use crate::game::game_evaluator::GameStats;

#[derive(Debug)]
pub struct GameEvaluationService {
    pool: PgPool,
    redis_client: Option<Arc<redis::Client>>,
    standings: StandingsService,
}

#[derive(Debug)]
pub struct EvaluationResult {
    pub games_evaluated: usize,
    pub games_updated: usize,
    pub errors: Vec<String>,
    pub game_results: HashMap<Uuid, GameStats>,
}

impl GameEvaluationService {
    pub fn new_with_redis(pool: PgPool, redis_client: Option<Arc<redis::Client>>) -> Self {
        Self { 
            standings: StandingsService::new(pool.clone()),
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
                id, season_id, home_team_id, away_team_id, scheduled_time, 
                week_number, is_first_leg, status as "status: GameStatus", 
                home_score_final, away_score_final, winner_team_id, week_start_date, week_end_date,
                created_at, updated_at
            FROM league_games 
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
            UPDATE league_games 
            SET 
                home_score_final = $2,
                away_score_final = $3,
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
        updated_game.home_score_final = Some(game_stats.home_team_score as i32);
        updated_game.away_score_final = Some(game_stats.away_team_score as i32);
        updated_game.winner_team_id = game_stats.winner_team_id;
        updated_game.status = GameStatus::Evaluated;

        // Update standings
        self.standings.update_after_game_result(
            &updated_game,
            game_stats.home_team_score as i32,
            game_stats.away_team_score as i32
        ).await?;

        tracing::info!("✅ Updated game {} and standings: {} - {}", 
            game_id, game_stats.home_team_score, game_stats.away_team_score);

        Ok(())
    }

    /// Evaluate and update finished live games
    pub async fn evaluate_finished_live_games(&self, game_ids: Vec<Uuid>) -> Result<Vec<GameStats>, sqlx::Error> {
        if game_ids.is_empty() {
            tracing::info!("🎯 No games to evaluate");
            return Ok(Vec::new());
        }
        tracing::info!("🎯 Evaluating finished live games: {:?}", game_ids);

        // Get the game details
        let games = sqlx::query!(
            r#"
            SELECT id, home_team_id, away_team_id
            FROM league_games 
            WHERE id = ANY($1) and status = 'finished'
            "#,
            &game_ids
        )
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();

        for game_data in games {
            let game_id = game_data.id;

            // Get the scores from live_games table and set later as final scores in league games table
            let live_game_scores = sqlx::query!(
                r#"
                SELECT home_score, away_score
                FROM live_games
                WHERE game_id = $1
                ORDER BY created_at DESC
                LIMIT 1
                "#,
                game_id
            )
            .fetch_optional(&self.pool)
            .await?;

            let game_stats = if let Some(live_scores) = live_game_scores {
                let winner_team_id = if live_scores.home_score > live_scores.away_score {
                    Some(game_data.home_team_id)
                } else if live_scores.away_score > live_scores.home_score {
                    Some(game_data.away_team_id)
                } else {
                    None
                };

                GameStats {
                    game_id,
                    home_team_name: String::new(),
                    away_team_name: String::new(),
                    home_team_score: live_scores.home_score as u32,
                    away_team_score: live_scores.away_score as u32,
                    home_team_result: if live_scores.home_score > live_scores.away_score { 
                        MatchResult::Win 
                    } else if live_scores.home_score < live_scores.away_score { 
                        MatchResult::Loss 
                    } else { 
                        MatchResult::Draw 
                    },
                    away_team_result: if live_scores.away_score > live_scores.home_score { 
                        MatchResult::Win 
                    } else if live_scores.away_score < live_scores.home_score { 
                        MatchResult::Loss 
                    } else { 
                        MatchResult::Draw 
                    },
                    winner_team_id,
                    home_score: live_scores.home_score as u32,
                    away_score: live_scores.away_score as u32,
                }
            } else {
                // This should not happen if all games are live games
                tracing::error!("❌ No live game data found for finished game {}", game_id);
                continue;
            };

            // Update the game result in the database
            match self.update_game_result(game_id, &game_stats).await {
                Ok(_) => {
                    tracing::info!("✅ Finished live game {} evaluated and updated: {} - {}", 
                        game_id, game_stats.home_team_score, game_stats.away_team_score);
                    results.push(game_stats);
                }
                Err(e) => {
                    tracing::error!("❌ Failed to update game {}: {}", game_id, e);
                }
            }
        }

        Ok(results)
    }

    /// Broadcast game evaluation results to all league participants via WebSocket
    async fn broadcast_game_evaluation_results(
        &self,
        game_results: &HashMap<Uuid, GameStats>,
        date: chrono::DateTime<Utc>,
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
                        title: format!("Match Result: {}", result_text),
                        message: format!(
                            "Your team scored {} against {} ({}). Final score: {} - {}",
                            user_team_score, opponent_name, opponent_score, user_team_score, opponent_score
                        ),
                        notification_type: NotificationType::GameResult,
                        action_url: Some(format!("/game/{}", game_id)),
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