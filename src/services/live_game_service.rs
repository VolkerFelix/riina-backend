use chrono::Utc;
use sqlx::{PgPool, Row};
use uuid::Uuid;
use tracing::{info, error, debug, warn};

use crate::models::live_game::{LiveGame, LiveGameScoreUpdate, LiveGameResponse};
use crate::models::game_events::GameEvent;
use crate::db::live_game_queries::LiveGameQueries;
use crate::services::game_evaluation_service::GameEvaluationService;
use redis::AsyncCommands;

#[derive(Debug)]
pub struct LiveGameService {
    pool: PgPool,
    live_game_queries: LiveGameQueries,
    redis_client: Option<redis::Client>,
    game_evaluation_service: GameEvaluationService,
}

impl LiveGameService {
    pub fn new(pool: PgPool, redis_client: Option<redis::Client>) -> Self {
        let redis_arc = redis_client.map(std::sync::Arc::new);
        Self {
            live_game_queries: LiveGameQueries::new(pool.clone()),
            game_evaluation_service: GameEvaluationService::new_with_redis(pool.clone(), redis_arc.clone()),
            pool,
            redis_client: redis_arc.map(|arc| (*arc).clone()),
        }
    }

    /// Initialize live games for all games that are starting now
    pub async fn initialize_starting_games(&self) -> Result<Vec<LiveGame>, Box<dyn std::error::Error>> {
        info!("Checking for games starting now to initialize live games");

        // Find games that are starting now (within last 5 minutes) but don't have live games yet
        let starting_games_query = "
            SELECT g.id as game_id
            FROM league_games g
            LEFT JOIN live_games lg ON g.id = lg.game_id AND lg.is_active = true
            WHERE g.status = 'in_progress'
            AND g.scheduled_time <= NOW()
            AND g.scheduled_time > NOW() - INTERVAL '5 minutes'
            AND lg.id IS NULL
        ";

        let game_rows = sqlx::query(starting_games_query)
            .fetch_all(&self.pool)
            .await?;

        let mut initialized_games = Vec::new();

        for row in game_rows {
            let game_id: Uuid = row.get("game_id");
            
            match self.initialize_live_game(game_id).await {
                Ok(live_game) => {
                    info!("Successfully initialized live game for game {}", game_id);
                    initialized_games.push(live_game);
                }
                Err(e) => {
                    error!("Failed to initialize live game for game {}: {}", game_id, e);
                }
            }
        }

        info!("Initialized {} live games", initialized_games.len());
        Ok(initialized_games)
    }

    /// Initialize a live game for a specific game
    pub async fn initialize_live_game(&self, game_id: Uuid) -> Result<LiveGame, Box<dyn std::error::Error>> {
        info!("Initializing live game for game_id: {}", game_id);

        // Check if live game already exists
        if let Some(existing_game) = self.live_game_queries.get_live_game_by_game_id(game_id).await? {
            warn!("Live game already exists for game {}", game_id);
            return Ok(existing_game);
        }

        // Create new live game
        let live_game = self.live_game_queries.create_live_game(game_id).await?;

        // Broadcast game started event
        if let Some(redis) = &self.redis_client {
            let event = GameEvent::LiveScoreUpdate {
                game_id: live_game.game_id,
                home_team_id: live_game.home_team_id,
                home_team_name: live_game.home_team_name.clone(),
                away_team_id: live_game.away_team_id,
                away_team_name: live_game.away_team_name.clone(),
                home_score: 0,
                away_score: 0,
                home_power: 0,
                away_power: 0,
                game_progress: 0.0,
                game_time_remaining: live_game.time_remaining(),
                is_active: true,
                last_updated: Utc::now(),
            };

            let mut conn = redis.get_async_connection().await?;
            let message = serde_json::to_string(&event)?;
            
            let global_channel = "game:events:global";
            let result: Result<i32, redis::RedisError> = conn.publish(global_channel, message).await;
            
            if let Err(e) = result {
                error!("Failed to broadcast game started event: {}", e);
            }
        }

        info!("Live game initialized: {} vs {}", 
            live_game.home_team_name, live_game.away_team_name);

        Ok(live_game)
    }

    /// Handle a score update from a player's workout data
    pub async fn handle_score_update(
        &self,
        game_id: Uuid,
        update: LiveGameScoreUpdate,
    ) -> Result<LiveGame, Box<dyn std::error::Error>> {
        debug!("Handling score update for game {} from user {}", game_id, update.username);

        // Get or create live game
        let live_game = match self.live_game_queries.get_live_game_by_game_id(game_id).await? {
            Some(game) => game,
            None => {
                info!("Live game doesn't exist for game {}, creating it", game_id);
                self.initialize_live_game(game_id).await?
            }
        };

        // Check if game is still active
        if !live_game.is_active || live_game.should_finish() {
            warn!("Attempted to update score for inactive game {}", game_id);
            
            // Mark game as finished if time is up
            if live_game.should_finish() {
                self.finish_live_game(live_game.id).await?;
            }
            
            return Err("Game is not active".into());
        }

        // Update the live game score
        let updated_game = self.live_game_queries
            .update_live_game_score(live_game.id, &update)
            .await?;

        // Broadcast the score update
        self.broadcast_live_score_update(&updated_game).await?;

        info!("Score updated for game {}: {} {} - {} {} (Player: {} +{})", 
            game_id,
            updated_game.home_team_name,
            updated_game.home_score,
            updated_game.away_score,
            updated_game.away_team_name,
            update.username,
            update.score_increase
        );

        Ok(updated_game)
    }

    /// Broadcast live score update to WebSocket clients
    async fn broadcast_live_score_update(
        &self,
        live_game: &LiveGame,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(redis_client) = &self.redis_client {
            let event = GameEvent::LiveScoreUpdate {
                game_id: live_game.game_id,
                home_team_id: live_game.home_team_id,
                home_team_name: live_game.home_team_name.clone(),
                away_team_id: live_game.away_team_id,
                away_team_name: live_game.away_team_name.clone(),
                home_score: live_game.home_score as u32,
                away_score: live_game.away_score as u32,
                home_power: live_game.home_power as u32,
                away_power: live_game.away_power as u32,
                game_progress: live_game.game_progress(),
                game_time_remaining: live_game.time_remaining(),
                is_active: live_game.is_active,
                last_updated: live_game.updated_at,
            };

            let mut conn = redis_client.get_async_connection().await?;
            let message = serde_json::to_string(&event)?;
            
            let global_channel = "game:events:global";
            let result: Result<i32, redis::RedisError> = conn.publish(global_channel, message).await;
            
            match result {
                Ok(receivers) => {
                    info!("📡 Broadcasted live score update to {} subscribers", receivers);
                }
                Err(e) => {
                    error!("❌ Failed to broadcast live score update: {}", e);
                    return Err(Box::new(e));
                }
            }
        }

        Ok(())
    }

    /// Get all active live games
    pub async fn get_active_live_games(&self) -> Result<Vec<LiveGame>, Box<dyn std::error::Error>> {
        let games = self.live_game_queries.get_active_live_games().await?;
        Ok(games)
    }

    /// Get complete live game data
    pub async fn get_live_game_response(
        &self,
        game_id: Uuid,
    ) -> Result<LiveGameResponse, Box<dyn std::error::Error>> {
        let live_game = self.live_game_queries
            .get_live_game_by_game_id(game_id)
            .await?
            .ok_or("Live game not found")?;

        let response = self.live_game_queries
            .get_live_game_response(live_game.id)
            .await?;

        Ok(response)
    }

    /// Finish live games that have ended
    pub async fn finish_ended_games(&self) -> Result<Vec<Uuid>, Box<dyn std::error::Error>> {
        info!("Checking for live games that should be finished");

        let ended_games_query = "
            SELECT id, game_id 
            FROM live_games 
            WHERE is_active = true 
            AND game_end_time <= NOW()
        ";

        let ended_rows = sqlx::query(ended_games_query)
            .fetch_all(&self.pool)
            .await?;

        let mut finished_games = Vec::new();

        for row in ended_rows {
            let live_game_id: Uuid = row.get("id");
            let game_id: Uuid = row.get("game_id");

            match self.finish_live_game(live_game_id).await {
                Ok(_) => {
                    info!("Successfully finished live game {}", game_id);
                    finished_games.push(game_id);
                }
                Err(e) => {
                    error!("Failed to finish live game {}: {}", game_id, e);
                }
            }
        }

        info!("Finished {} live games", finished_games.len());
        Ok(finished_games)
    }

    /// Finish a specific live game
    pub async fn finish_live_game(&self, live_game_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        // Get the live game before finishing to get the game_id
        let live_game = sqlx::query!(
            "SELECT game_id FROM live_games WHERE id = $1",
            live_game_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(live_game_row) = live_game {
            let game_id = live_game_row.game_id;
            
            // First finish the live game (updates both live_games and league_games tables)
            self.live_game_queries.finish_live_game(live_game_id).await?;
            info!("Live game {} finished", live_game_id);

            // Then evaluate the game and update standings
            match self.game_evaluation_service.evaluate_finished_live_game(game_id).await {
                Ok(game_stats) => {
                    info!("✅ Game {} evaluated successfully: {} - {}", 
                        game_id, game_stats.home_team_score, game_stats.away_team_score);
                }
                Err(e) => {
                    // Log the error but don't fail the entire operation
                    // The game status has already been updated
                    warn!("⚠️ Failed to evaluate finished game {}: {}. Game status updated but standings may need manual update.", 
                        game_id, e);
                }
            }
        } else {
            warn!("Live game {} not found when trying to finish", live_game_id);
        }

        Ok(())
    }

    /// Check if a user is participating in any active live games
    pub async fn get_user_active_games(&self, user_id: Uuid) -> Result<Vec<LiveGame>, Box<dyn std::error::Error>> {
        let user_games_query = "
            SELECT DISTINCT 
                lg.id, lg.game_id, lg.home_team_id, lg.home_team_name, lg.away_team_id, lg.away_team_name,
                lg.home_score, lg.away_score, lg.home_power, lg.away_power,
                lg.game_start_time, lg.game_end_time, lg.last_score_time, lg.last_scorer_id,
                lg.last_scorer_name, lg.last_scorer_team, lg.is_active, lg.created_at, lg.updated_at
            FROM live_games lg
            JOIN live_player_contributions lpc ON lg.id = lpc.live_game_id
            JOIN league_games g ON lg.game_id = g.id
            WHERE lpc.user_id = $1 
            AND lg.is_active = true
            AND g.status = 'in_progress'
            AND lg.game_start_time <= NOW()
            AND lg.game_end_time > NOW()
        ";

        let games = sqlx::query_as!(
            LiveGame,
            r#"
            SELECT 
                lg.id, lg.game_id, lg.home_team_id, lg.home_team_name, lg.away_team_id, lg.away_team_name,
                lg.home_score, lg.away_score, lg.home_power, lg.away_power,
                lg.game_start_time, lg.game_end_time, lg.last_score_time, lg.last_scorer_id,
                lg.last_scorer_name, lg.last_scorer_team, lg.is_active, lg.created_at, lg.updated_at
            FROM live_games lg
            JOIN live_player_contributions lpc ON lg.id = lpc.live_game_id
            WHERE lpc.user_id = $1 
            AND lg.is_active = true
            AND lg.game_start_time <= NOW()
            AND lg.game_end_time > NOW()
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(games)
    }

    /// Calculate score increase from stat changes
    pub fn calculate_score_from_stats(&self, stamina_gained: i32, strength_gained: i32) -> i32 {
        // Simple scoring: each stat point = 1 score point
        // Could be made more sophisticated with different weights
        stamina_gained + strength_gained
    }

    /// Calculate power increase from stat changes
    pub fn calculate_power_from_stats(&self, stamina_gained: i32, strength_gained: i32) -> i32 {
        // Power is just the sum of current stats
        stamina_gained + strength_gained
    }
}