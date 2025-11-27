// Removed unused imports: use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use tracing::info;

use crate::models::league::{LeagueGame, GameStatus, LiveGameScoreUpdate};

#[derive(Debug)]
pub struct GameQueries {
    pool: PgPool,
}

impl GameQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Start a game (update status to InProgress - times are already set by scheduling)
    pub async fn start_game(&self, game_id: Uuid) -> Result<(), sqlx::Error> {
        info!("Starting game: {}", game_id);

        sqlx::query!(
            r#"
            UPDATE games 
            SET 
                status = 'in_progress',
                updated_at = NOW()
            WHERE id = $1
            "#,
            game_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Calculate team scores from live_score_events using best 4 out of 5 players
    /// This is a public method that can be called to recalculate scores for a game
    pub async fn calculate_team_scores_best_4(&self, game_id: Uuid) -> Result<(i32, i32), sqlx::Error> {
        // Get all player scores grouped by team and user
        let player_scores = sqlx::query!(
            r#"
            SELECT
                lse.team_side,
                lse.user_id,
                SUM(lse.score_points) as total_score
            FROM live_score_events lse
            WHERE lse.game_id = $1
            GROUP BY lse.team_side, lse.user_id
            ORDER BY lse.team_side, total_score DESC
            "#,
            game_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut home_scores: Vec<f64> = Vec::new();
        let mut away_scores: Vec<f64> = Vec::new();

        for player in player_scores {
            let score = player.total_score.unwrap_or(0.0) as f64;
            match player.team_side.as_str() {
                "home" => home_scores.push(score),
                "away" => away_scores.push(score),
                _ => {}
            }
        }

        // Sort in descending order and take best 4
        home_scores.sort_by(|a, b| b.partial_cmp(a).unwrap());
        away_scores.sort_by(|a, b| b.partial_cmp(a).unwrap());

        let home_score: i32 = home_scores.iter().take(4).sum::<f64>() as i32;
        let away_score: i32 = away_scores.iter().take(4).sum::<f64>() as i32;

        info!("Calculated team scores for game {}: home={} (best {} of {}), away={} (best {} of {})",
            game_id, home_score, home_scores.len().min(4), home_scores.len(),
            away_score, away_scores.len().min(4), away_scores.len());

        Ok((home_score, away_score))
    }

    /// Update game score from a workout
    pub async fn update_game_score(
        &self,
        game_id: Uuid,
        update: &LiveGameScoreUpdate,
    ) -> Result<(), sqlx::Error> {
        info!("Processing score update for game {} from user {}", game_id, update.username);

        // Verify the user is part of one of the teams
        let game_info = sqlx::query!(
            r#"
            SELECT
                g.home_team_id,
                g.away_team_id,
                CASE
                    WHEN tm.team_id = g.home_team_id THEN 'home'
                    WHEN tm.team_id = g.away_team_id THEN 'away'
                    ELSE 'unknown'
                END as "team_side!"
            FROM games g
            JOIN team_members tm ON (tm.team_id = g.home_team_id OR tm.team_id = g.away_team_id)
            JOIN users u ON tm.user_id = u.id
            WHERE g.id = $1 AND u.id = $2 AND tm.status = 'active'
            "#,
            game_id,
            update.user_id
        )
        .fetch_optional(&self.pool)
        .await?;

        let game_info = match game_info {
            Some(info) => info,
            None => {
                info!("User {} is not a member of teams playing in game {}", update.user_id, game_id);
                return Ok(());
            }
        };

        // NOTE: Score is already recorded in live_score_events by the caller
        // Now we recalculate team totals from live_score_events (best 4 out of 5)
        let (home_score, away_score) = self.calculate_team_scores_best_4(game_id).await?;

        // Update game with new calculated scores
        sqlx::query!(
            r#"
            UPDATE games
            SET
                home_score = $2,
                away_score = $3,
                last_score_time = NOW(),
                last_scorer_id = $4,
                last_scorer_name = $5,
                last_scorer_team = $6,
                updated_at = NOW()
            WHERE id = $1
            "#,
            game_id,
            home_score,
            away_score,
            update.user_id,
            update.username,
            game_info.team_side
        )
        .execute(&self.pool)
        .await?;

        info!("✅ Score updated for game {} by {} ({}): home={}, away={}",
            game_id, update.username, game_info.team_side, home_score, away_score);

        Ok(())
    }

    /// Finish a game (update status to Finished and set game_end_time)
    pub async fn finish_game(&self, game_id: Uuid) -> Result<(), sqlx::Error> {
        info!("Finishing game: {}", game_id);

        sqlx::query!(
            r#"
            UPDATE games 
            SET 
                status = 'finished',
                updated_at = NOW()
            WHERE id = $1
            "#,
            game_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get active games (status = 'in_progress')
    pub async fn get_active_games(&self) -> Result<Vec<LeagueGame>, sqlx::Error> {
        let games = sqlx::query_as!(
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
            WHERE status = 'in_progress'
            ORDER BY game_start_time ASC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(games)
    }

    /// Get games that need to be started
    pub async fn get_games_ready_to_start(&self) -> Result<Vec<LeagueGame>, sqlx::Error> {
        let games = sqlx::query_as!(
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
            WHERE status = 'scheduled' 
            AND game_start_time <= CURRENT_TIMESTAMP
            ORDER BY game_start_time ASC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(games)
    }

    /// Get games that need to be finished
    pub async fn get_completed_games(&self) -> Result<Vec<LeagueGame>, sqlx::Error> {
        // First, let's see what games are in_progress and their times
        let in_progress_games = sqlx::query!(
            r#"
            SELECT id, game_start_time, game_end_time, 
                   CURRENT_TIMESTAMP as now,
                   (game_end_time <= CURRENT_TIMESTAMP) as should_finish
            FROM games 
            WHERE status = 'in_progress'
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        for game in &in_progress_games {
            info!("🔍 In-progress game {}: start={:?}, end={:?}, now={:?}, should_finish={:?}", 
                game.id, game.game_start_time, game.game_end_time, game.now, game.should_finish);
        }

        let games = sqlx::query_as!(
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
            WHERE status = 'in_progress' 
            AND game_end_time <= CURRENT_TIMESTAMP
            ORDER BY game_start_time ASC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        info!("🔍 Found {} games ready to finish out of {} in-progress games", 
            games.len(), in_progress_games.len());

        Ok(games)
    }
}