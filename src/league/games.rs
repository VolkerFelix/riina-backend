use chrono::Datelike;
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


    /// Get next upcoming game for a season
    pub async fn get_next_game(&self, season_id: Uuid) -> Result<Option<GameWithTeams>, sqlx::Error> {
        let now = chrono::Utc::now();
        
        let game_query = sqlx::query!(
            r#"
            SELECT 
                lg.*,
                'Team ' || SUBSTRING(lg.home_team_id::text, 1, 8) as home_team_name,
                'Team ' || SUBSTRING(lg.away_team_id::text, 1, 8) as away_team_name,
                '#E74C3C' as home_team_color,
                '#3498DB' as away_team_color
            FROM league_games lg
            WHERE lg.season_id = $1 
            AND lg.status = 'scheduled'
            AND lg.scheduled_time >= $2
            ORDER BY lg.scheduled_time ASC
            LIMIT 1
            "#,
            season_id,
            now
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(game_query.map(|row| {
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

    /// Get games scheduled for this week
    pub async fn get_games_this_week(&self, season_id: Uuid) -> Result<Vec<GameWithTeams>, sqlx::Error> {
        let now = chrono::Utc::now();
        let week_start = now - chrono::Duration::days(now.weekday().num_days_from_monday() as i64);
        let week_end = week_start + chrono::Duration::days(7);

        let games_query = sqlx::query!(
            r#"
            SELECT 
                lg.*,
                'Team ' || SUBSTRING(lg.home_team_id::text, 1, 8) as home_team_name,
                'Team ' || SUBSTRING(lg.away_team_id::text, 1, 8) as away_team_name,
                '#E74C3C' as home_team_color,
                '#3498DB' as away_team_color
            FROM league_games lg
            WHERE lg.season_id = $1 
            AND lg.scheduled_time >= $2
            AND lg.scheduled_time < $3
            ORDER BY lg.scheduled_time ASC
            "#,
            season_id,
            week_start,
            week_end
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(games_query.into_iter().map(|row| {
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
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                },
                home_team_name: row.home_team_name.unwrap_or_default(),
                away_team_name: row.away_team_name.unwrap_or_default(),
                home_team_color: row.home_team_color.unwrap_or_default(),
                away_team_color: row.away_team_color.unwrap_or_default(),
            }
        }).collect())
    }

    /// Get upcoming games for a season with optional limit
    pub async fn get_upcoming_games(&self, season_id: Uuid, limit: Option<i64>) -> Result<Vec<GameWithTeams>, sqlx::Error> {
        let now = chrono::Utc::now();
        let limit = limit.unwrap_or(10); // Default to 10 games if no limit specified

        let games_query = sqlx::query!(
            r#"
            SELECT 
                lg.*,
                'Team ' || SUBSTRING(lg.home_team_id::text, 1, 8) as home_team_name,
                'Team ' || SUBSTRING(lg.away_team_id::text, 1, 8) as away_team_name,
                '#E74C3C' as home_team_color,
                '#3498DB' as away_team_color
            FROM league_games lg
            WHERE lg.season_id = $1 
            AND lg.scheduled_time >= $2
            AND lg.status IN ('scheduled', 'postponed')
            ORDER BY lg.scheduled_time ASC
            LIMIT $3
            "#,
            season_id,
            now,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(games_query.into_iter().map(|row| {
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
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                },
                home_team_name: row.home_team_name.unwrap_or_default(),
                away_team_name: row.away_team_name.unwrap_or_default(),
                home_team_color: row.home_team_color.unwrap_or_default(),
                away_team_color: row.away_team_color.unwrap_or_default(),
            }
        }).collect())
    }
}