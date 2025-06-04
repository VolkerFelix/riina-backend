use chrono::{DateTime, Utc, Duration};
use sqlx::PgPool;
use uuid::Uuid;
use crate::models::league::*;

/// Service responsible for individual game operations
pub struct GameService {
    pool: PgPool,
}

impl GameService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Update game result and return the updated game
    pub async fn update_result(
        &self,
        game_id: Uuid,
        home_score: i32,
        away_score: i32,
    ) -> Result<LeagueGame, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // Determine winner
        let winner_team_id = if home_score > away_score {
            Some("home_team_id")
        } else if away_score > home_score {
            Some("away_team_id")
        } else {
            None // Draw
        };

        // Update the game with result
        let updated_game = if let Some(winner_col) = winner_team_id {
            sqlx::query_as!(
                LeagueGame,
                r#"
                UPDATE league_games 
                SET home_score = $1, 
                    away_score = $2, 
                    status = 'finished',
                    winner_team_id = CASE 
                        WHEN $3 = 'home_team_id' THEN home_team_id
                        ELSE away_team_id
                    END,
                    updated_at = NOW()
                WHERE id = $4
                RETURNING *
                "#,
                home_score,
                away_score,
                winner_col,
                game_id
            )
            .fetch_one(&mut *tx)
            .await?
        } else {
            // Draw - no winner
            sqlx::query_as!(
                LeagueGame,
                r#"
                UPDATE league_games 
                SET home_score = $1, 
                    away_score = $2, 
                    status = 'finished',
                    winner_team_id = NULL,
                    updated_at = NOW()
                WHERE id = $3
                RETURNING *
                "#,
                home_score,
                away_score,
                game_id
            )
            .fetch_one(&mut *tx)
            .await?
        };

        tx.commit().await?;

        tracing::info!(
            "Updated game {}: {} - {} (winner: {:?})",
            game_id,
            home_score,
            away_score,
            updated_game.winner_team_id
        );

        Ok(updated_game)
    }

    /// Get a specific game by ID
    pub async fn get_game(&self, game_id: Uuid) -> Result<Option<LeagueGame>, sqlx::Error> {
        sqlx::query_as!(
            LeagueGame,
            "SELECT * FROM league_games WHERE id = $1",
            game_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Get game with team information
    pub async fn get_game_with_teams(&self, game_id: Uuid) -> Result<Option<GameWithTeams>, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            SELECT 
                lg.*,
                'Team ' || SUBSTRING(lg.home_team_id::text, 1, 8) as home_team_name,
                'Team ' || SUBSTRING(lg.away_team_id::text, 1, 8) as away_team_name,
                '#E74C3C' as home_team_color,
                '#3498DB' as away_team_color
            FROM league_games lg
            WHERE lg.id = $1
            "#,
            game_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|row| {
            let status = match row.status.as_str() {
                "live" => GameStatus::Live,
                "finished" => GameStatus::Finished,
                "postponed" => GameStatus::Postponed,
                _ => GameStatus::Scheduled,
            };

            GameWithTeams {
                game: LeagueGame {
                    id: row.id,
                    season_id: row.season_id,
                    home_team_id: row.home_team_id,
                    away_team_id: row.away_team_id,
                    scheduled_time: row.scheduled_time,
                    week_number: row.week_number,
                    is_first_leg: row.is_first_leg,
                    status,
                    home_score: row.home_score,
                    away_score: row.away_score,
                    winner_team_id: row.winner_team_id,
                    match_data: row.match_data.map(|data| sqlx::types::Json(data)),
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                },
                home_team_name: row.home_team_name.unwrap_or_default(),
                away_team_name: row.away_team_name.unwrap_or_default(),
                home_team_color: row.home_team_color.unwrap_or_default(),
                away_team_color: row.away_team_color.unwrap_or_default(),
            }
        }))
    }
}