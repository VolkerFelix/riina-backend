use chrono::{DateTime, Utc, Duration};
use sqlx::PgPool;
use uuid::Uuid;
use crate::models::league::*;
use crate::utils::team_power;
use super::timing::TimingService;

/// Service responsible for league schedule management
pub struct ScheduleService {
    pool: PgPool,
    timing: TimingService,
}

impl ScheduleService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            timing: TimingService::new(),
        }
    }

    /// Generate complete league schedule using round-robin algorithm
    /// Each team plays every other team twice (home and away)
    /// N/2 games happen simultaneously each week
    pub async fn generate_schedule(
        &self,
        season_id: Uuid,
        team_ids: &[Uuid],
        season_start_date: DateTime<Utc>,
    ) -> Result<i32, sqlx::Error> {
        let team_count = team_ids.len();
        if team_count < 2 {
            tracing::error!("Cannot create schedule with less than 2 teams");
            return Err(sqlx::Error::RowNotFound);
        }

        // Get the season's game duration to calculate game end times
        let season = sqlx::query!(
            "SELECT game_duration_minutes FROM league_seasons WHERE id = $1",
            season_id
        )
        .fetch_one(&self.pool)
        .await?;
        
        let game_duration_minutes = season.game_duration_minutes;
        let game_duration_seconds = (game_duration_minutes * 60.0) as i64;
        let game_duration = Duration::seconds(game_duration_seconds);


        let games_per_round = team_count / 2;
        tracing::info!("Generating round-robin schedule for {} teams, {} games per round", team_count, games_per_round);

        let mut tx = self.pool.begin().await?;
        let mut games_created = 0;

        // Use the circle method for round-robin scheduling
        // This guarantees perfect scheduling with no conflicts
        let mut teams: Vec<usize> = (0..team_count).collect();
        
        // For the circle method to work with home/away balance,
        // we'll generate all rounds twice (once normal, once with home/away swapped)
        
        // FIRST LEG: Generate N-1 rounds
        for round in 0..(team_count - 1) {
            let round_counter_for_readability = round + 1;
            let game_start_time = self.timing.calculate_game_start_time(season_start_date, round, game_duration)?;
            
            // Generate pairings for this round
            for i in 0..games_per_round {
                let home_idx = if i == 0 {
                    // First team stays fixed
                    0
                } else {
                    teams[i]
                };
                
                let away_idx = teams[team_count - 1 - i];
                
                let home_team = team_ids[home_idx];
                let away_team = team_ids[away_idx];
                
                tracing::debug!(
                    "First leg - Round {}: {} (home) vs {} (away)",
                    round_counter_for_readability, home_team, away_team
                );

                // Round starts at the scheduled time, ends after game duration
                let game_end_time = game_start_time.clone() + game_duration;

                sqlx::query!(
                    r#"
                    INSERT INTO games (
                        season_id, home_team_id, away_team_id, 
                        week_number, is_first_leg, status, game_start_time, game_end_time
                    ) VALUES ($1, $2, $3, $4, TRUE, 'scheduled', $5, $6)
                    "#,
                    season_id,
                    home_team,
                    away_team,
                    round_counter_for_readability as i32,
                    game_start_time,
                    game_end_time
                )
                .execute(&mut *tx)
                .await?;
                
                games_created += 1;
            }
            
            // Rotate teams for next round (except first team which stays fixed)
            let last = teams.pop().unwrap();
            teams.insert(1, last);
        }
        
        tracing::info!("Completed first leg: {} games in {} weeks", games_created, team_count - 1);
        
        // Reset teams array for second leg
        teams = (0..team_count).collect();
        
        // SECOND LEG: Generate N-1 rounds with home/away swapped
        for round in 0..(team_count - 1) {
            let game_round = (team_count - 1) + round;
            let round_counter_for_readability = game_round + 1;
            let game_start_time = self.timing.calculate_game_start_time(season_start_date, game_round, game_duration)?;
            
            // Generate pairings for this round (with home/away swapped)
            for i in 0..games_per_round {
                let home_idx = if i == 0 {
                    // First team stays fixed but now plays away
                    teams[team_count - 1 - i]
                } else {
                    teams[team_count - 1 - i]
                };
                
                let away_idx = if i == 0 {
                    0
                } else {
                    teams[i]
                };
                
                let home_team = team_ids[home_idx];
                let away_team = team_ids[away_idx];
                
                tracing::debug!(
                    "Second leg - Round {}: {} (home) vs {} (away)",
                    game_round + 1, home_team, away_team
                );

                // Week starts at the scheduled time, ends after game duration
                let game_end_time = game_start_time.clone() + game_duration;

                sqlx::query!(
                    r#"
                    INSERT INTO games (
                        season_id, home_team_id, away_team_id,
                        week_number, is_first_leg, status, game_start_time, game_end_time
                    ) VALUES ($1, $2, $3, $4, FALSE, 'scheduled', $5, $6)
                    "#,
                    season_id,
                    home_team,
                    away_team,
                    round_counter_for_readability as i32,
                    game_start_time,
                    game_end_time
                )
                .execute(&mut *tx)
                .await?;
                
                games_created += 1;
            }
            
            // Rotate teams for next round (except first team which stays fixed)
            let last = teams.pop().unwrap();
            teams.insert(1, last);
        }
        
        tracing::info!("Completed second leg: {} total games in {} rounds", games_created, 2 * (team_count - 1));

        tx.commit().await?;

        let total_weeks = 2 * (team_count - 1);
        tracing::info!(
            "Schedule generation complete: {} total games over {} rounds ({} games per round)",
            games_created,
            total_weeks,
            games_per_round
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
        
        // Log info about games found
        tracing::info!("Found {} games for season {}, total weeks: {}", games_with_teams.len(), season_id, total_weeks);
        
        let next_game_time = self.timing.get_next_game_time();

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
                ht.team_name as home_team_name,
                at.team_name as away_team_name,
                ht.team_color as home_team_color,
                at.team_color as away_team_color
            FROM games lg
            JOIN teams ht ON lg.home_team_id = ht.id
            JOIN teams at ON lg.away_team_id = at.id
            WHERE lg.season_id = $1 AND lg.week_number = $2
            ORDER BY lg.game_start_time ASC
            "#,
            season_id,
            week_number
        )
        .fetch_all(&self.pool)
        .await?;

        if games_query.is_empty() {
            return Err(sqlx::Error::RowNotFound);
        }

        let game_time = games_query[0].game_start_time.unwrap_or_else(|| chrono::Utc::now());
        let now = Utc::now();
        let next_saturday = self.timing.get_next_game_time();
        let is_current_week = (game_time - next_saturday).abs() < Duration::days(7);
        
        let countdown_seconds = if is_current_week && game_time > now {
            Some((game_time - now).num_seconds().max(0))
        } else {
            None
        };

        // Collect all unique team IDs for power calculation
        let mut team_ids = Vec::new();
        for row in &games_query {
            team_ids.push(row.home_team_id);
            team_ids.push(row.away_team_id);
        }
        team_ids.sort();
        team_ids.dedup();

        // Calculate team powers for all teams
        let team_powers = team_power::calculate_multiple_team_powers(&team_ids, &self.pool).await?;

        // Convert query results to GameWithTeams with team powers
        let games_with_teams = games_query.into_iter().map(|row| {
            let status = match row.status.as_str() {
                "live" => GameStatus::InProgress,
                "finished" => GameStatus::Finished,
                "postponed" => GameStatus::Postponed,
                _ => GameStatus::Scheduled,
            };

            GameWithTeams {
                game: LeagueGame::with_defaults(
                    row.id,
                    row.season_id,
                    row.home_team_id,
                    row.away_team_id,
                    row.week_number,
                    row.is_first_leg,
                    status,
                    row.winner_team_id,
                    row.created_at,
                    row.updated_at,
                ),
                home_team_name: row.home_team_name,
                away_team_name: row.away_team_name,
                home_team_color: row.home_team_color,
                away_team_color: row.away_team_color,
                home_team_power: team_powers.get(&row.home_team_id).copied(),
                away_team_power: team_powers.get(&row.away_team_id).copied(),
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
                ht.team_name as home_team_name,
                at.team_name as away_team_name,
                ht.team_color as home_team_color,
                at.team_color as away_team_color
            FROM games lg
            JOIN teams ht ON lg.home_team_id = ht.id
            JOIN teams at ON lg.away_team_id = at.id
            WHERE lg.season_id = $1 
            AND lg.status = 'scheduled'
            AND lg.game_start_time >= $2
            ORDER BY lg.game_start_time ASC
            LIMIT $3
            "#,
            season_id,
            now,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        // Collect all unique team IDs for power calculation
        let mut team_ids = Vec::new();
        for row in &games_query {
            team_ids.push(row.home_team_id);
            team_ids.push(row.away_team_id);
        }
        team_ids.sort();
        team_ids.dedup();

        // Calculate team powers for all teams
        let team_powers = team_power::calculate_multiple_team_powers(&team_ids, &self.pool).await?;

        Ok(games_query.into_iter().map(|row| {
            let status = match row.status.as_str() {
                "live" => GameStatus::InProgress,
                "finished" => GameStatus::Finished,
                "postponed" => GameStatus::Postponed,
                _ => GameStatus::Scheduled,
            };

            GameWithTeams {
                game: LeagueGame::with_defaults(
                    row.id,
                    row.season_id,
                    row.home_team_id,
                    row.away_team_id,
                    row.week_number,
                    row.is_first_leg,
                    status,
                    row.winner_team_id,
                    row.created_at,
                    row.updated_at,
                ),
                home_team_name: row.home_team_name,
                away_team_name: row.away_team_name,
                home_team_color: row.home_team_color,
                away_team_color: row.away_team_color,
                home_team_power: team_powers.get(&row.home_team_id).copied(),
                away_team_power: team_powers.get(&row.away_team_id).copied(),
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
                ht.team_name as home_team_name,
                at.team_name as away_team_name,
                ht.team_color as home_team_color,
                at.team_color as away_team_color
            FROM games lg
            JOIN teams ht ON lg.home_team_id = ht.id
            JOIN teams at ON lg.away_team_id = at.id
            WHERE lg.season_id = $1 
            AND lg.status = 'finished'
            ORDER BY lg.game_start_time DESC
            LIMIT $2
            "#,
            season_id,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        // Collect all unique team IDs for power calculation
        let mut team_ids = Vec::new();
        for row in &games_query {
            team_ids.push(row.home_team_id);
            team_ids.push(row.away_team_id);
        }
        team_ids.sort();
        team_ids.dedup();

        // Calculate team powers for all teams
        let team_powers = team_power::calculate_multiple_team_powers(&team_ids, &self.pool).await?;

        Ok(games_query.into_iter().map(|row| {
            let status = match row.status.as_str() {
                "live" => GameStatus::InProgress,
                "finished" => GameStatus::Finished,
                "postponed" => GameStatus::Postponed,
                _ => GameStatus::Scheduled,
            };

            GameWithTeams {
                game: LeagueGame::with_defaults(
                    row.id,
                    row.season_id,
                    row.home_team_id,
                    row.away_team_id,
                    row.week_number,
                    row.is_first_leg,
                    status,
                    row.winner_team_id,
                    row.created_at,
                    row.updated_at,
                ),
                home_team_name: row.home_team_name,
                away_team_name: row.away_team_name,
                home_team_color: row.home_team_color,
                away_team_color: row.away_team_color,
                home_team_power: team_powers.get(&row.home_team_id).copied(),
                away_team_power: team_powers.get(&row.away_team_id).copied(),
            }
        }).collect())
    }

    /// Calculate total number of weeks needed for a league with N teams
    /// Formula: Total games ÷ Games per week = N*(N-1) ÷ (N/2) = 2*(N-1)
    pub fn calculate_total_weeks(&self, team_count: usize) -> i32 {
        if team_count < 2 {
            return 0;
        }
        // Teams are guaranteed to be even due to validation
        (2 * (team_count - 1)) as i32
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

        // Allow any start date - the schedule will automatically adjust to Saturday 10pm for actual games
        // No restriction on start date format - season can begin at any time

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
                lg.id,
                lg.season_id,
                lg.home_team_id,
                lg.away_team_id,
                lg.game_start_time,
                lg.game_end_time,
                lg.week_number,
                lg.is_first_leg,
                lg.status,
                lg.winner_team_id,
                lg.created_at,
                lg.updated_at,
                lg.home_score,
                lg.away_score,
                lg.last_score_time,
                lg.last_scorer_id,
                lg.last_scorer_name,
                lg.last_scorer_team,
                ht.team_name as home_team_name,
                at.team_name as away_team_name,
                ht.team_color as home_team_color,
                at.team_color as away_team_color
            FROM games lg
            JOIN teams ht ON lg.home_team_id = ht.id
            JOIN teams at ON lg.away_team_id = at.id
            WHERE lg.season_id = $1
            ORDER BY lg.game_start_time, lg.week_number
            "#,
            season_id
        )
        .fetch_all(&self.pool)
        .await?;

        // Collect all unique team IDs for power calculation
        let mut team_ids = Vec::new();
        for row in &games_query {
            team_ids.push(row.home_team_id);
            team_ids.push(row.away_team_id);
        }
        team_ids.sort();
        team_ids.dedup();

        // Calculate team powers for all teams
        let team_powers = team_power::calculate_multiple_team_powers(&team_ids, &self.pool).await?;

        Ok(games_query.into_iter().map(|row| {
            let status = match row.status.as_str() {
                "live" => GameStatus::InProgress,
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
                    week_number: row.week_number,
                    is_first_leg: row.is_first_leg,
                    status,
                    winner_team_id: row.winner_team_id,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    home_score: row.home_score,
                    away_score: row.away_score,
                    game_start_time: row.game_start_time,
                    game_end_time: row.game_end_time,
                    last_score_time: row.last_score_time,
                    last_scorer_id: row.last_scorer_id,
                    last_scorer_name: row.last_scorer_name,
                    last_scorer_team: row.last_scorer_team,
                },
                home_team_name: row.home_team_name,
                away_team_name: row.away_team_name,
                home_team_color: row.home_team_color,
                away_team_color: row.away_team_color,
                home_team_power: team_powers.get(&row.home_team_id).copied(),
                away_team_power: team_powers.get(&row.away_team_id).copied(),
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
                MIN(game_start_time) as first_game_time,
                MAX(game_start_time) as last_game_time,
                MIN(week_number) as first_week,
                MAX(week_number) as last_week
            FROM games
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
                ht.team_name as home_team_name,
                at.team_name as away_team_name,
                ht.team_color as home_team_color,
                at.team_color as away_team_color
            FROM games lg
            JOIN teams ht ON lg.home_team_id = ht.id
            JOIN teams at ON lg.away_team_id = at.id
            WHERE lg.season_id = $1 
            AND lg.game_start_time >= $2
            AND lg.game_start_time <= $3
            ORDER BY lg.game_start_time ASC
            "#,
            season_id,
            start_utc,
            end_utc
        )
        .fetch_all(&self.pool)
        .await?;

        // Collect all unique team IDs for power calculation
        let mut team_ids = Vec::new();
        for row in &games_query {
            team_ids.push(row.home_team_id);
            team_ids.push(row.away_team_id);
        }
        team_ids.sort();
        team_ids.dedup();

        // Calculate team powers for all teams
        let team_powers = team_power::calculate_multiple_team_powers(&team_ids, &self.pool).await?;

        Ok(games_query.into_iter().map(|row| {
            let status = match row.status.as_str() {
                "live" => GameStatus::InProgress,
                "finished" => GameStatus::Finished,
                "postponed" => GameStatus::Postponed,
                _ => GameStatus::Scheduled,
            };

            GameWithTeams {
                game: LeagueGame::with_defaults(
                    row.id,
                    row.season_id,
                    row.home_team_id,
                    row.away_team_id,
                    row.week_number,
                    row.is_first_leg,
                    status,
                    row.winner_team_id,
                    row.created_at,
                    row.updated_at,
                ),
                home_team_name: row.home_team_name,
                away_team_name: row.away_team_name,
                home_team_color: row.home_team_color,
                away_team_color: row.away_team_color,
                home_team_power: team_powers.get(&row.home_team_id).copied(),
                away_team_power: team_powers.get(&row.away_team_id).copied(),
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