// Removed unused imports: use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use tracing::{info, debug};

use crate::models::league::{LeagueGame, GameStatus, LiveGameScoreUpdate};

#[derive(Debug)]
pub struct GameQueries {
    pool: PgPool,
}

impl GameQueries {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Start a game (update status to InProgress and set game_start_time)
    pub async fn start_game(&self, game_id: Uuid) -> Result<(), sqlx::Error> {
        info!("Starting game: {}", game_id);

        sqlx::query!(
            r#"
            UPDATE games 
            SET 
                status = 'in_progress',
                game_start_time = NOW(),
                updated_at = NOW()
            WHERE id = $1
            "#,
            game_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update game score from a workout
    pub async fn update_game_score(
        &self,
        game_id: Uuid,
        update: &LiveGameScoreUpdate,
    ) -> Result<(), sqlx::Error> {
        debug!("Updating score for game {} from user {}", game_id, update.username);

        // Get current scores and determine which team this user is on
        let game_info = sqlx::query!(
            r#"
            SELECT 
                g.home_team_id,
                g.away_team_id,
                g.home_score,
                g.away_score,
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

        // Update the appropriate score
        match game_info.team_side.as_str() {
            "home" => {
                sqlx::query!(
                    r#"
                    UPDATE games 
                    SET 
                        home_score = home_score + $2,
                        last_score_time = NOW(),
                        last_scorer_id = $3,
                        last_scorer_name = $4,
                        last_scorer_team = 'home',
                        updated_at = NOW()
                    WHERE id = $1
                    "#,
                    game_id,
                    update.score_increase,
                    update.user_id,
                    update.username
                )
                .execute(&self.pool)
                .await?;
            }
            "away" => {
                sqlx::query!(
                    r#"
                    UPDATE games 
                    SET 
                        away_score = away_score + $2,
                        last_score_time = NOW(),
                        last_scorer_id = $3,
                        last_scorer_name = $4,
                        last_scorer_team = 'away',
                        updated_at = NOW()
                    WHERE id = $1
                    "#,
                    game_id,
                    update.score_increase,
                    update.user_id,
                    update.username
                )
                .execute(&self.pool)
                .await?;
            }
            _ => {
                info!("User {} team side unknown for game {}", update.user_id, game_id);
                return Ok(());
            }
        }

        info!("Score updated for game {}: +{} points by {} ({})", 
            game_id, update.score_increase, update.username, game_info.team_side);

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
                game_end_time = NOW(),
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
    pub async fn get_pending_games(&self) -> Result<Vec<LeagueGame>, sqlx::Error> {
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

        Ok(games)
    }
}