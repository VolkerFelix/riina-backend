use chrono::{DateTime, Utc, NaiveDate};
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;

use crate::models::league::{LeagueGame, GameStatus};
use crate::game::game_evaluator::{GameEvaluator, GameStats};

/// Service for managing week-long games
pub struct WeekGameService {
    pool: PgPool,
}

impl WeekGameService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Start games that should be in progress (today is within their week window)
    pub async fn start_due_games(&self) -> Result<Vec<Uuid>, sqlx::Error> {
        let started_games = sqlx::query!(
            r#"
            UPDATE league_games 
            SET status = 'in_progress', updated_at = NOW()
            WHERE status = 'scheduled'
            AND CURRENT_DATE BETWEEN week_start_date AND week_end_date
            RETURNING id
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let game_ids: Vec<Uuid> = started_games.into_iter().map(|row| row.id).collect();
        
        if !game_ids.is_empty() {
            tracing::info!("🎮 Started {} week-long games", game_ids.len());
        }

        Ok(game_ids)
    }

    /// Finish games where the week has ended
    pub async fn finish_completed_games(&self) -> Result<Vec<Uuid>, sqlx::Error> {
        // Get games that need to be finished (week has ended)
        let games_to_finish = sqlx::query!(
            r#"
            SELECT id, home_team_id, away_team_id, week_start_date, week_end_date
            FROM league_games 
            WHERE status = 'in_progress'
            AND CURRENT_DATE > week_end_date
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut finished_game_ids = Vec::new();

        for game in games_to_finish {
            // Calculate final scores
            let game_stats = GameEvaluator::calculate_live_score(
                &self.pool,
                &game.home_team_id,
                &game.away_team_id,
                game.week_start_date,
                game.week_end_date
            ).await?;

            // Update game with final results
            sqlx::query!(
                r#"
                UPDATE league_games 
                SET 
                    status = 'finished',
                    home_score = $1,
                    away_score = $2,
                    winner_team_id = $3,
                    updated_at = NOW()
                WHERE id = $4
                "#,
                game_stats.home_score as i32,
                game_stats.away_score as i32,
                game_stats.winner_team_id,
                game.id
            )
            .execute(&self.pool)
            .await?;

            finished_game_ids.push(game.id);
            tracing::info!("🏁 Finished game {} with scores {} - {}", 
                game.id, game_stats.home_score, game_stats.away_score);
        }

        Ok(finished_game_ids)
    }

    /// Get live scores for all currently active games
    pub async fn get_live_scores(&self) -> Result<Vec<(Uuid, GameStats)>, sqlx::Error> {
        GameEvaluator::get_live_scores_for_active_games(&self.pool).await
    }

    /// Get live score for a specific game
    pub async fn get_game_live_score(&self, game_id: Uuid) -> Result<Option<GameStats>, sqlx::Error> {
        let game = sqlx::query!(
            r#"
            SELECT 
                id, home_team_id, away_team_id, week_start_date, week_end_date, status,
                ht.team_name as home_team_name,
                at.team_name as away_team_name
            FROM league_games lg
            JOIN teams ht ON lg.home_team_id = ht.id
            JOIN teams at ON lg.away_team_id = at.id
            WHERE lg.id = $1
            AND lg.status = 'in_progress'
            "#,
            game_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(game) = game {
            let mut game_stats = GameEvaluator::calculate_live_score(
                &self.pool,
                &game.home_team_id,
                &game.away_team_id,
                game.week_start_date,
                game.week_end_date
            ).await?;

            // Add game details
            game_stats.game_id = game.id;
            game_stats.home_team_name = game.home_team_name;
            game_stats.away_team_name = game.away_team_name;

            Ok(Some(game_stats))
        } else {
            Ok(None)
        }
    }

    /// Get all games that are currently in their active week
    pub async fn get_active_games(&self) -> Result<Vec<LeagueGame>, sqlx::Error> {
        let games = sqlx::query_as!(
            LeagueGame,
            r#"
            SELECT 
                id, season_id, home_team_id, away_team_id, scheduled_time,
                week_number, is_first_leg, status as "status: GameStatus",
                home_score, away_score, winner_team_id, week_start_date, week_end_date,
                created_at, updated_at
            FROM league_games
            WHERE status = 'in_progress'
            AND CURRENT_DATE BETWEEN week_start_date AND week_end_date
            ORDER BY week_number, scheduled_time
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(games)
    }

    /// Run the game management cycle (start new games, finish completed ones)
    pub async fn run_game_cycle(&self) -> Result<(Vec<Uuid>, Vec<Uuid>), sqlx::Error> {
        let started_games = self.start_due_games().await?;
        let finished_games = self.finish_completed_games().await?;
        
        Ok((started_games, finished_games))
    }
}