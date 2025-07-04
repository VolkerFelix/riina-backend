use sqlx::PgPool;
use uuid::Uuid;

use crate::models::league::{LeagueGame, GameStatus};
use crate::game::game_evaluator::GameEvaluator;

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
        // First, get games that need to be started with their team info
        let games_to_start = sqlx::query!(
            r#"
            SELECT id, home_team_id, away_team_id
            FROM league_games 
            WHERE status = 'scheduled'
            AND CURRENT_DATE BETWEEN week_start_date AND week_end_date
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut started_game_ids = Vec::new();

        for game in games_to_start {
            // Take start snapshots for both teams
            match GameEvaluator::take_team_snapshot(&self.pool, &game.id, &game.home_team_id, "start").await {
                Ok(_) => tracing::debug!("📸 Took start snapshot for home team {} in game {}", game.home_team_id, game.id),
                Err(e) => tracing::error!("Failed to take start snapshot for home team {}: {:?}", game.home_team_id, e),
            }

            match GameEvaluator::take_team_snapshot(&self.pool, &game.id, &game.away_team_id, "start").await {
                Ok(_) => tracing::debug!("📸 Took start snapshot for away team {} in game {}", game.away_team_id, game.id),
                Err(e) => tracing::error!("Failed to take start snapshot for away team {}: {:?}", game.away_team_id, e),
            }

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

            started_game_ids.push(game.id);
            tracing::info!("🎮 Started week-long game {} with snapshots taken", game.id);
        }
        
        if !started_game_ids.is_empty() {
            tracing::info!("🎮 Started {} week-long games", started_game_ids.len());
        }

        Ok(started_game_ids)
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
            // Take end snapshots for both teams
            match GameEvaluator::take_team_snapshot(&self.pool, &game.id, &game.home_team_id, "end").await {
                Ok(_) => tracing::debug!("📸 Took end snapshot for home team {} in game {}", game.home_team_id, game.id),
                Err(e) => tracing::error!("Failed to take end snapshot for home team {}: {:?}", game.home_team_id, e),
            }

            match GameEvaluator::take_team_snapshot(&self.pool, &game.id, &game.away_team_id, "end").await {
                Ok(_) => tracing::debug!("📸 Took end snapshot for away team {} in game {}", game.away_team_id, game.id),
                Err(e) => tracing::error!("Failed to take end snapshot for away team {}: {:?}", game.away_team_id, e),
            }

            // Calculate final scores using snapshots
            let game_stats = GameEvaluator::evaluate_game_with_snapshots(
                &self.pool,
                &game.id,
                &game.home_team_id,
                &game.away_team_id
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
            tracing::info!("🏁 Finished game {} with snapshot-based scores {} - {}", 
                game.id, game_stats.home_score, game_stats.away_score);
        }

        Ok(finished_game_ids)
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