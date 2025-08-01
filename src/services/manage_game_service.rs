use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;

use crate::models::league::{LeagueGame, GameStatus};
use crate::services::LiveGameService;

/// Service for managing games in a season
pub struct ManageGameService {
    pool: PgPool,
    live_game_service: LiveGameService,
}

impl ManageGameService {
    pub fn new(pool: PgPool) -> Self {
        let live_game_service = LiveGameService::new(pool.clone(), None);
        Self { pool, live_game_service }
    }

    pub fn new_with_redis(pool: PgPool, redis_client: Option<Arc<redis::Client>>) -> Self {
        let live_game_service = LiveGameService::new(pool.clone(), redis_client);
        Self { pool, live_game_service }
    }

    /// Start games that should be in progress (current time is within their week window)
    pub async fn start_due_games(&self) -> Result<Vec<Uuid>, sqlx::Error> {
        // Get games that need to be started with their team info
        // Use CURRENT_TIMESTAMP to handle different game durations properly
        let games_to_start = sqlx::query!(
            r#"
            SELECT id, home_team_id, away_team_id
            FROM league_games 
            WHERE status = 'scheduled'
            AND CURRENT_TIMESTAMP BETWEEN week_start_date AND week_end_date
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut started_game_ids = Vec::new();

        for game in games_to_start {
            // Update game status
            sqlx::query!(
                r#"
                UPDATE league_games 
                SET status = 'in_progress', updated_at = NOW()
                WHERE id = $1
                "#,
                game.id
            )
            .execute(&self.pool)
            .await?;

            // Initialize live game for real-time scoring
            match self.live_game_service.initialize_live_game(game.id).await {
                Ok(_) => tracing::info!("✅ Initialized live game for game {}", game.id),
                Err(e) => {
                    let error_msg = format!("{:?}", e);
                    tracing::error!("❌ Failed to initialize live game for {}: {}", game.id, error_msg);
                }
            }

            started_game_ids.push(game.id);
            tracing::info!("🎮 Started week-long game {} with live scoring", game.id);
        }
        
        if !started_game_ids.is_empty() {
            tracing::info!("🎮 Started {} week-long games", started_game_ids.len());
        }

        Ok(started_game_ids)
    }

    /// Finish games where the game has ended
    pub async fn finish_completed_games(&self) -> Result<Vec<Uuid>, sqlx::Error> {
        // Get games that need to be finished
        let games_to_finish = sqlx::query!(
            r#"
            SELECT id, home_team_id, away_team_id, week_start_date, week_end_date
            FROM league_games 
            WHERE status = 'in_progress'
            AND CURRENT_TIMESTAMP > week_end_date
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut finished_game_ids = Vec::new();

        for game in games_to_finish {
            // Finish the live game (this will mark it as inactive)
            match self.live_game_service.finish_live_game(game.id).await {
                Ok(_) => {
                    finished_game_ids.push(game.id);
                    tracing::info!("🏁 Finished game {} - live scoring will be used for final evaluation", game.id);
                }
                Err(e) => {
                    let error_msg = format!("{:?}", e);
                    tracing::error!("❌ Failed to finish live game {}: {}", game.id, error_msg);
                    // Still mark the league game as finished even if live game finishing fails
                    sqlx::query!(
                        "UPDATE league_games SET status = 'finished', updated_at = NOW() WHERE id = $1",
                        game.id
                    )
                    .execute(&self.pool)
                    .await?;
                    finished_game_ids.push(game.id);
                }
            }
        }

        Ok(finished_game_ids)
    }


    /// Get all games that are currently in their active period
    pub async fn get_active_games(&self) -> Result<Vec<LeagueGame>, sqlx::Error> {
        let games = sqlx::query_as!(
            LeagueGame,
            r#"
            SELECT 
                id, season_id, home_team_id, away_team_id, scheduled_time,
                week_number, is_first_leg, status as "status: GameStatus",
                home_score_final, away_score_final, winner_team_id, week_start_date, week_end_date,
                created_at, updated_at
            FROM league_games
            WHERE status = 'in_progress'
            AND CURRENT_TIMESTAMP BETWEEN week_start_date AND week_end_date
            ORDER BY week_number, scheduled_time
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(games)
    }

    async fn get_pending_games(&self) -> Result<Vec<LeagueGame>, sqlx::Error> {
        let games = sqlx::query_as!(
            LeagueGame,
            r#"
            SELECT
                id, season_id, home_team_id, away_team_id, scheduled_time,
                week_number, is_first_leg, status as "status: GameStatus",
                home_score_final, away_score_final, winner_team_id, week_start_date, week_end_date,
                created_at, updated_at
            FROM league_games
            WHERE status = 'scheduled'
            AND scheduled_time > CURRENT_TIMESTAMP
            ORDER BY scheduled_time
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(games)
    }

    /// Run the game management cycle (start new games, finish completed ones)
    pub async fn run_game_cycle(&self) -> Result<(Vec<Uuid>, Vec<Uuid>, Vec<Uuid>, Vec<Uuid>), sqlx::Error> {
        let pending_games = self.get_pending_games().await?.iter().map(|game| game.id).collect();
        let live_games = self.get_active_games().await?.iter().map(|game| game.id).collect();
        let started_games = self.start_due_games().await?;
        let finished_games = self.finish_completed_games().await?;

        
        Ok((pending_games, live_games, started_games, finished_games))
    }
}