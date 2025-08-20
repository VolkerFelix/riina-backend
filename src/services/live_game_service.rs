use chrono::Utc;
use sqlx::{PgPool, Row};
use uuid::Uuid;
use tracing::{info, error, debug, warn};
use std::sync::Arc;

use crate::models::live_game::{LiveGame, LiveGameScoreUpdate, LiveGameResponse};
use crate::models::game_events::GameEvent;
use crate::db::live_game_queries::LiveGameQueries;
use redis::AsyncCommands;

#[derive(Debug)]
pub struct LiveGameService {
    pool: PgPool,
    live_game_queries: LiveGameQueries,
    redis_client: Arc<redis::Client>,
}

impl LiveGameService {
    pub fn new(pool: PgPool, redis_client: Arc<redis::Client>) -> Self {
        Self {
            live_game_queries: LiveGameQueries::new(pool.clone()),
            pool,
            redis_client,
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
    pub async fn initialize_live_game(&self, game_id: Uuid) -> Result<LiveGame, sqlx::Error> {
        info!("Initializing live game for game_id: {}", game_id);

        // Check if live game already exists
        if let Some(existing_game) = self.live_game_queries.get_live_game_by_game_id(game_id).await? {
            warn!("Live game already exists for game {}", game_id);
            return Ok(existing_game);
        }

        // Create new live game
        let live_game = self.live_game_queries.create_live_game(game_id).await?;

        // Broadcast game started event
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

            match self.redis_client.get_async_connection().await {
                Ok(mut conn) => {
                    match serde_json::to_string(&event) {
                        Ok(message) => {
                            let global_channel = "game:events:global";
                            let result: Result<i32, redis::RedisError> = conn.publish(global_channel, message).await;
                            
                            if let Err(e) = result {
                                error!("Failed to broadcast game started event: {}", e);
                            }
                        }
                        Err(e) => error!("Failed to serialize game started event: {}", e),
                    }
                }
            Err(e) => error!("Failed to get Redis connection: {}", e),
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

    /// Get a live game by its ID
    pub async fn get_live_game_by_id(&self, live_game_id: Uuid) -> Result<Option<LiveGame>, sqlx::Error> {
        self.live_game_queries.get_live_game_by_id(live_game_id).await
    }

    /// Broadcast live score update to WebSocket clients
    pub async fn broadcast_live_score_update(
        &self,
        live_game: &LiveGame,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

            let mut conn = self.redis_client.get_async_connection().await?;
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

    /// Finish a specific live game
    pub async fn finish_live_game(&self, live_game_id: Uuid) -> Result<(), sqlx::Error> {
        self.live_game_queries.finish_live_game(live_game_id).await?;
        info!("Live game {} finished", live_game_id);

        Ok(())
    }

    /// Check if a user is participating in any active live games
    pub async fn get_user_active_games(&self, user_id: Uuid) -> Result<Vec<LiveGame>, Box<dyn std::error::Error>> {
        let games = sqlx::query_as!(
            LiveGame,
            r#"
            SELECT 
                lg.id, lg.game_id, lg.home_team_id, lg.home_team_name, lg.away_team_id, lg.away_team_name,
                lg.home_score, lg.away_score, lg.home_power, lg.away_power,
                lg.game_start_time, lg.game_end_time, lg.last_score_time, lg.last_scorer_id,
                lg.last_scorer_name, lg.last_scorer_team, lg.is_active, lg.created_at, lg.updated_at
            FROM live_games lg
            JOIN team_members tm ON (tm.team_id = lg.home_team_id OR tm.team_id = lg.away_team_id)
            WHERE tm.user_id = $1 
            AND tm.status = 'active'
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