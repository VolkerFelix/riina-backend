use chrono::{DateTime, Utc, Duration};
use sqlx::PgPool;
use uuid::Uuid;
use crate::models::league::*;
use super::countdown::CountdownService;

/// Service responsible for league schedule management
pub struct ScheduleService {
    pool: PgPool,
    countdown: CountdownService,
}

impl ScheduleService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            countdown: CountdownService::new(),
        }
    }

    /// Generate complete league schedule using round-robin algorithm
    /// Each team plays every other team twice (home and away)
    pub async fn generate_schedule(
        &self,
        season_id: Uuid,
        team_ids: &[Uuid],
        start_date: DateTime<Utc>,
    ) -> Result<i32, sqlx::Error> {
        let team_count = team_ids.len();
        if team_count < 2 {
            tracing::warn!("Cannot create schedule with less than 2 teams");
            return Ok(0);
        }

        tracing::info!("Generating round-robin schedule for {} teams", team_count);

        let mut tx = self.pool.begin().await?;
        let mut games_created = 0;
        let mut week_num = 1;

        // FIRST LEG: Each team plays every other team once
        for i in 0..team_count {
            for j in (i + 1)..team_count {
                let game_time = self.countdown.calculate_game_time_for_week(start_date, week_num);
                
                tracing::debug!(
                    "Creating first leg: Week {} - Team {} (home) vs Team {} (away) at {}",
                    week_num, 
                    team_ids[i], 
                    team_ids[j], 
                    game_time
                );

                sqlx::query!(
                    r#"
                    INSERT INTO league_games (
                        season_id, home_team_id, away_team_id, scheduled_time, 
                        week_number, is_first_leg, status
                    ) VALUES ($1, $2, $3, $4, $5, TRUE, 'scheduled')
                    "#,
                    season_id,
                    team_ids[i],    // Home team
                    team_ids[j],    // Away team
                    game_time,
                    week_num
                )
                .execute(&mut *tx)
                .await?;
                
                games_created += 1;
                week_num += 1;
            }
        }

        tracing::info!("Created {} first-leg games", games_created);

        // SECOND LEG: Return fixtures (swap home and away)
        for i in 0..team_count {
            for j in (i + 1)..team_count {
                let game_time = self.countdown.calculate_game_time_for_week(start_date, week_num);
                
                tracing::debug!(
                    "Creating return fixture: Week {} - Team {} (home) vs Team {} (away) at {}",
                    week_num, 
                    team_ids[j], 
                    team_ids[i], 
                    game_time
                );

                sqlx::query!(
                    r#"
                    INSERT INTO league_games (
                        season_id, home_team_id, away_team_id, scheduled_time,
                        week_number, is_first_leg, status
                    ) VALUES ($1, $2, $3, $4, $5, FALSE, 'scheduled')
                    "#,
                    season_id,
                    team_ids[j],    // Home team (swapped)
                    team_ids[i],    // Away team (swapped)
                    game_time,
                    week_num
                )
                .execute(&mut *tx)
                .await?;
                
                games_created += 1;
                week_num += 1;
            }
        }

        tx.commit().await?;

        tracing::info!(
            "Schedule generation complete: {} total games over {} weeks",
            games_created,
            week_num - 1
        );

        Ok(games_created)
    }

    /// Get complete season schedule with team details
    pub async fn get_season_schedule(
        &self,
        season_id: Uuid,
    ) -> Result<LeagueScheduleResponse, sqlx::Error> {
        let season = sqlx::query_as!(
            LeagueSeason,
            "SELECT * FROM league_seasons WHERE id = $1",
            season_id
        )
        .fetch_one(&self.pool)
        .await?;

        let games_with_teams = self.get_games_with_team_info(season_id).await?;

        let total_weeks = games_with_teams.iter()
            .map(|g| g.game.week_number)
            .max()
            .unwrap_or(0);
        
        let next_game_time = self.countdown.get_next_game_time();

        Ok(LeagueScheduleResponse {
            season,
            games: games_with_teams,
            next_game_time,
            total_weeks,
        })
    }

    /// Get games for a specific week
    pub async fn get_game_week(
        &self,
        season_id: Uuid,
        week_number: i32,
    ) -> Result<GameWeekResponse, sqlx::Error> {
        let games_query = sqlx::query!(
            r#"
            SELECT 
                lg.*,
                'Team ' || SUBSTRING(lg.home_team_id::text, 1, 8) as home_team_name,
                'Team ' || SUBSTRING(lg.away_team_id::text, 1, 8) as away_team_name,
                '#E74C3C' as home_team_color,
                '#3498DB' as away_team_color
            FROM league_games lg
            WHERE lg.season_id = $1 AND lg.week_number = $2
            ORDER BY lg.scheduled_time ASC
            "#,
            season_id,
            week_number
        )
        .fetch_all(&self.pool)
        .await?;

        if games_query.is_empty() {
            return Err(sqlx::Error::RowNotFound);
        }

        let game_time = games_query[0].scheduled_time;
        let now = Utc::now();
        let next_saturday = self.countdown.get_next_game_time();
        let is_current_week = (game_time - next_saturday).abs() < Duration::days(7);
        
        let countdown_seconds = if is_current_week && game_time > now {
            Some((game_time - now).num_seconds().max(0))
        } else {
            None
        };

        // Convert query results to GameWithTeams directly
        let games_with_teams = games_query.into_iter().map(|row| {
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
        }).collect();

        Ok(GameWeekResponse {
            week_number,
            game_time,
            games: games_with_teams,
            is_current_week,
            countdown_seconds,
        })
    }

    /// Get upcoming games (next N games)
    pub async fn get_upcoming_games(
        &self,
        season_id: Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<GameWithTeams>, sqlx::Error> {
        let limit = limit.unwrap_or(5);
        let now = Utc::now();

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
            AND lg.status = 'scheduled'
            AND lg.scheduled_time >= $2
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

    /// Get recent results (last N completed games)
    pub async fn get_recent_results(
        &self,
        season_id: Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<GameWithTeams>, sqlx::Error> {
        let limit = limit.unwrap_or(5);

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
            AND lg.status = 'finished'
            ORDER BY lg.scheduled_time DESC
            LIMIT $2
            "#,
            season_id,
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

    /// Calculate total number of weeks needed for a league with N teams
    pub fn calculate_total_weeks(&self, team_count: usize) -> i32 {
        if team_count < 2 {
            return 0;
        }
        // Each team plays every other team twice: (n-1) * 2 weeks
        ((team_count - 1) * 2) as i32
    }

    /// Calculate total number of games in a complete season
    pub fn calculate_total_games(&self, team_count: usize) -> i32 {
        if team_count < 2 {
            return 0;
        }
        // Each team plays every other team twice: n * (n-1) total games
        (team_count * (team_count - 1)) as i32
    }

    /// Validate schedule parameters
    pub fn validate_schedule_parameters(
        &self,
        team_count: usize,
        start_date: DateTime<Utc>,
    ) -> Result<(), String> {
        if team_count < 2 {
            return Err("Minimum 2 teams required".to_string());
        }

        if team_count > 20 {
            return Err("Maximum 20 teams allowed".to_string());
        }

        // Check if start date is valid (should be a Saturday)
        if !self.countdown.is_valid_game_time(start_date) {
            return Err("Start date must be a Saturday at 22:00 UTC".to_string());
        }

        let total_weeks = self.calculate_total_weeks(team_count);
        let end_date = start_date + Duration::weeks(total_weeks as i64);
        let max_reasonable_duration = Duration::weeks(52); // 1 year max

        if end_date - start_date > max_reasonable_duration {
            return Err("Season duration would exceed 1 year".to_string());
        }

        Ok(())
    }

    /// Get all games with team information for a season
    async fn get_games_with_team_info(&self, season_id: Uuid) -> Result<Vec<GameWithTeams>, sqlx::Error> {
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
            ORDER BY lg.scheduled_time, lg.week_number
            "#,
            season_id
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

    /// Get schedule statistics for a season
    pub async fn get_schedule_statistics(&self, season_id: Uuid) -> Result<ScheduleStatistics, sqlx::Error> {
        let stats = sqlx::query!(
            r#"
            SELECT 
                COUNT(*) as total_games,
                SUM(CASE WHEN status = 'finished' THEN 1 ELSE 0 END) as completed_games,
                SUM(CASE WHEN status = 'scheduled' THEN 1 ELSE 0 END) as upcoming_games,
                SUM(CASE WHEN status = 'live' THEN 1 ELSE 0 END) as live_games,
                SUM(CASE WHEN status = 'postponed' THEN 1 ELSE 0 END) as postponed_games,
                MIN(scheduled_time) as first_game_time,
                MAX(scheduled_time) as last_game_time,
                MIN(week_number) as first_week,
                MAX(week_number) as last_week
            FROM league_games
            WHERE season_id = $1
            "#,
            season_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(ScheduleStatistics {
            total_games: stats.total_games.unwrap_or(0) as i32,
            completed_games: stats.completed_games.unwrap_or(0) as i32,
            upcoming_games: stats.upcoming_games.unwrap_or(0) as i32,
            live_games: stats.live_games.unwrap_or(0) as i32,
            postponed_games: stats.postponed_games.unwrap_or(0) as i32,
            first_game_time: stats.first_game_time,
            last_game_time: stats.last_game_time,
            first_week: stats.first_week.unwrap_or(1),
            last_week: stats.last_week.unwrap_or(1),
            progress_percentage: if stats.total_games.unwrap_or(0) > 0 {
                (stats.completed_games.unwrap_or(0) as f32 / stats.total_games.unwrap_or(1) as f32) * 100.0
            } else {
                0.0
            }
        })
    }

    /// Get games happening on a specific date
    pub async fn get_games_on_date(
        &self,
        season_id: Uuid,
        date: chrono::NaiveDate,
    ) -> Result<Vec<GameWithTeams>, sqlx::Error> {
        let start_of_day = date.and_hms_opt(0, 0, 0).unwrap();
        let end_of_day = date.and_hms_opt(23, 59, 59).unwrap();
        
        let start_utc: DateTime<Utc> = DateTime::from_naive_utc_and_offset(start_of_day, Utc);
        let end_utc: DateTime<Utc> = DateTime::from_naive_utc_and_offset(end_of_day, Utc);

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
            AND lg.scheduled_time <= $3
            ORDER BY lg.scheduled_time ASC
            "#,
            season_id,
            start_utc,
            end_utc
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

/// Statistics about a league schedule
#[derive(Debug, Clone)]
pub struct ScheduleStatistics {
    pub total_games: i32,
    pub completed_games: i32,
    pub upcoming_games: i32,
    pub live_games: i32,
    pub postponed_games: i32,
    pub first_game_time: Option<DateTime<Utc>>,
    pub last_game_time: Option<DateTime<Utc>>,
    pub first_week: i32,
    pub last_week: i32,
    pub progress_percentage: f32,
}