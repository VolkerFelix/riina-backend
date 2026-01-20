use sqlx::PgPool;
use uuid::Uuid;
use crate::models::league::*;
use crate::utils::team_power;
use std::collections::HashMap;

/// Service responsible for managing league standings
#[derive(Debug)]
pub struct StandingsService {
    pool: PgPool,
}

impl StandingsService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Initialize standings for a new season
    pub async fn initialize_for_season(
        &self,
        season_id: Uuid,
        team_ids: &[Uuid],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        
        for (position, team_id) in team_ids.iter().enumerate() {
            sqlx::query!(
                r#"
                INSERT INTO league_standings (
                    season_id, team_id, position
                ) VALUES ($1, $2, $3)
                ON CONFLICT (season_id, team_id) DO NOTHING
                "#,
                season_id,
                team_id,
                (position + 1) as i32
            )
            .execute(&mut *tx)
            .await?;
        }
        
        tx.commit().await?;
        tracing::info!("Initialized standings for season {} with {} teams", season_id, team_ids.len());
        Ok(())
    }

    /// Update standings after a game result
    pub async fn update_after_game_result(
        &self,
        game: &LeagueGame,
        home_score: i32,
        away_score: i32,
    ) -> Result<(), sqlx::Error> {
        tracing::info!("🏆 Updating standings for game {}: {} - {} (home team: {}, away team: {})", 
            game.id, home_score, away_score, game.home_team_id, game.away_team_id);

        let mut tx = self.pool.begin().await?;

        // Determine points for each team
        let (home_points, away_points) = if home_score > away_score {
            (3, 0) // Home win
        } else if away_score > home_score {
            (0, 3) // Away win
        } else {
            (1, 1) // Draw
        };

        tracing::info!("🏆 Points awarded: home team {} gets {} points, away team {} gets {} points", 
            game.home_team_id, home_points, game.away_team_id, away_points);

        // Update home team standings - use UPSERT to handle missing records
        let _home_result = sqlx::query!(
            r#"
            INSERT INTO league_standings (season_id, team_id, games_played, wins, draws, losses, position, last_updated)
            VALUES ($2, $3, 1, 
                CASE WHEN $1 = 3 THEN 1 ELSE 0 END,
                CASE WHEN $1 = 1 THEN 1 ELSE 0 END,
                CASE WHEN $1 = 0 THEN 1 ELSE 0 END,
                1, NOW())
            ON CONFLICT (season_id, team_id) DO UPDATE SET
                games_played = league_standings.games_played + 1,
                wins = league_standings.wins + CASE WHEN $1 = 3 THEN 1 ELSE 0 END,
                draws = league_standings.draws + CASE WHEN $1 = 1 THEN 1 ELSE 0 END,
                losses = league_standings.losses + CASE WHEN $1 = 0 THEN 1 ELSE 0 END,
                last_updated = NOW()
            "#,
            home_points,
            game.season_id,
            game.home_team_id
        )
        .execute(&mut *tx)
        .await?;


        // Update away team standings - use UPSERT to handle missing records
        let _away_result = sqlx::query!(
            r#"
            INSERT INTO league_standings (season_id, team_id, games_played, wins, draws, losses, position, last_updated)
            VALUES ($2, $3, 1, 
                CASE WHEN $1 = 3 THEN 1 ELSE 0 END,
                CASE WHEN $1 = 1 THEN 1 ELSE 0 END,
                CASE WHEN $1 = 0 THEN 1 ELSE 0 END,
                1, NOW())
            ON CONFLICT (season_id, team_id) DO UPDATE SET
                games_played = league_standings.games_played + 1,
                wins = league_standings.wins + CASE WHEN $1 = 3 THEN 1 ELSE 0 END,
                draws = league_standings.draws + CASE WHEN $1 = 1 THEN 1 ELSE 0 END,
                losses = league_standings.losses + CASE WHEN $1 = 0 THEN 1 ELSE 0 END,
                last_updated = NOW()
            "#,
            away_points,
            game.season_id,
            game.away_team_id
        )
        .execute(&mut *tx)
        .await?;


        // Recalculate positions
        self.recalculate_positions_in_tx(&mut tx, game.season_id).await?;

        tx.commit().await?;
        
        tracing::info!(
            "Updated standings after game {}: {}({}) - {}({})",
            game.id, game.home_team_id, home_points, game.away_team_id, away_points
        );
        
        Ok(())
    }

    /// Get league standings for a season
    pub async fn get_league_standings(&self, season_id: Uuid) -> Result<LeagueStandingsResponse, sqlx::Error> {
        let season = sqlx::query_as!(
            LeagueSeason,
            "SELECT * FROM league_seasons WHERE id = $1",
            season_id
        )
        .fetch_one(&self.pool)
        .await?;

        let standings_with_teams = sqlx::query!(
            r#"
            SELECT
                ls.*,
                t.team_name,
                t.team_color
            FROM league_standings ls
            JOIN teams t ON ls.team_id = t.id
            WHERE ls.season_id = $1
            ORDER BY ls.position ASC
            "#,
            season_id
        )
        .fetch_all(&self.pool)
        .await?;

        // Calculate total points scored for each team
        let game_rows = sqlx::query!(
            r#"
            SELECT
                home_team_id,
                away_team_id,
                home_score,
                away_score
            FROM games
            WHERE season_id = $1 AND status = 'finished'
            "#,
            season_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut total_points_scored: HashMap<Uuid, i32> = HashMap::new();
        for game in &game_rows {
            *total_points_scored.entry(game.home_team_id).or_insert(0) += game.home_score;
            *total_points_scored.entry(game.away_team_id).or_insert(0) += game.away_score;
        }

        // Calculate team powers
        let team_ids: Vec<Uuid> = standings_with_teams.iter().map(|row| row.team_id).collect();
        let team_powers = team_power::calculate_multiple_team_powers(&team_ids, &self.pool).await?;

        let standings: Vec<StandingWithTeam> = standings_with_teams
            .into_iter()
            .map(|row| {
                StandingWithTeam {
                    standing: LeagueStanding {
                        id: row.id,
                        season_id: row.season_id,
                        team_id: row.team_id,
                        games_played: row.games_played,
                        wins: row.wins,
                        draws: row.draws,
                        losses: row.losses,
                        points: row.points,
                        position: row.position,
                        last_updated: row.last_updated,
                        total_points_scored: total_points_scored.get(&row.team_id).copied(),
                    },
                    team_name: row.team_name,
                    team_color: row.team_color,
                    recent_form: vec!['W', 'L', 'D'], // TODO: Calculate actual form
                    team_power: team_powers.get(&row.team_id).copied().unwrap_or(0.0),
                }
            })
            .collect();

        let last_updated = standings
            .iter()
            .map(|s| s.standing.last_updated)
            .max()
            .unwrap_or_else(chrono::Utc::now);

        Ok(LeagueStandingsResponse {
            season,
            standings,
            last_updated,
        })
    }

    /// Recalculate all positions based on current points with tie-breaker logic
    async fn recalculate_positions_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        season_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        // Define a struct to hold game data
        #[derive(Clone)]
        struct GameData {
            home_team_id: Uuid,
            away_team_id: Uuid,
            home_score: i32,
            away_score: i32,
        }

        // Helper function to compare head-to-head records
        fn compare_head_to_head(
            team_a_id: Uuid,
            team_b_id: Uuid,
            games: &[GameData],
        ) -> std::cmp::Ordering {
            let mut a_points = 0;
            let mut b_points = 0;

            // Calculate head-to-head points from games between these two teams
            for game_data in games {
                // Check if this game involves both teams
                if (game_data.home_team_id == team_a_id && game_data.away_team_id == team_b_id)
                    || (game_data.home_team_id == team_b_id && game_data.away_team_id == team_a_id)
                {
                    let (a_score, b_score) = if game_data.home_team_id == team_a_id {
                        (game_data.home_score, game_data.away_score)
                    } else {
                        (game_data.away_score, game_data.home_score)
                    };

                    // Award points based on result
                    if a_score > b_score {
                        a_points += 3;
                    } else if b_score > a_score {
                        b_points += 3;
                    } else {
                        a_points += 1;
                        b_points += 1;
                    }
                }
            }

            // Compare head-to-head points (higher is better, so b first in comparison)
            b_points.cmp(&a_points)
        }

        // Get all standings with complete data
        let mut standings = sqlx::query!(
            r#"
            SELECT
                team_id,
                points,
                wins,
                draws,
                losses,
                games_played
            FROM league_standings
            WHERE season_id = $1
            "#,
            season_id
        )
        .fetch_all(&mut **tx)
        .await?;

        // Get all finished games for this season to calculate head-to-head and total points
        let game_rows = sqlx::query!(
            r#"
            SELECT
                home_team_id,
                away_team_id,
                home_score,
                away_score
            FROM games
            WHERE season_id = $1 AND status = 'finished'
            "#,
            season_id
        )
        .fetch_all(&mut **tx)
        .await?;

        // Convert to our simple struct
        let games: Vec<GameData> = game_rows
            .iter()
            .map(|row| GameData {
                home_team_id: row.home_team_id,
                away_team_id: row.away_team_id,
                home_score: row.home_score,
                away_score: row.away_score,
            })
            .collect();

        // Calculate total points scored for each team
        let mut total_points: HashMap<Uuid, i32> = HashMap::new();
        for game in &games {
            *total_points.entry(game.home_team_id).or_insert(0) += game.home_score;
            *total_points.entry(game.away_team_id).or_insert(0) += game.away_score;
        }

        // Sort standings using tie-breaker logic
        standings.sort_by(|a, b| {
            let a_points = a.points.unwrap_or(0);
            let b_points = b.points.unwrap_or(0);

            // 1. First by points
            let points_cmp = b_points.cmp(&a_points);
            if points_cmp != std::cmp::Ordering::Equal {
                return points_cmp;
            }

            // 2. Then by head-to-head record
            let h2h_cmp = compare_head_to_head(a.team_id, b.team_id, &games);
            if h2h_cmp != std::cmp::Ordering::Equal {
                return h2h_cmp;
            }

            // 3. Then by total points scored during the season
            let a_total = total_points.get(&a.team_id).copied().unwrap_or(0);
            let b_total = total_points.get(&b.team_id).copied().unwrap_or(0);
            b_total.cmp(&a_total)
        });

        // Update positions
        for (index, standing) in standings.iter().enumerate() {
            sqlx::query!(
                r#"
                UPDATE league_standings
                SET position = $1
                WHERE season_id = $2 AND team_id = $3
                "#,
                (index + 1) as i32,
                season_id,
                standing.team_id
            )
            .execute(&mut **tx)
            .await?;
        }

        Ok(())
    }

    /// Get standings for a specific team
    pub async fn get_team_standing(
        &self,
        season_id: Uuid,
        team_id: Uuid,
    ) -> Result<Option<LeagueStanding>, sqlx::Error> {
        let standing_row = sqlx::query!(
            "SELECT * FROM league_standings WHERE season_id = $1 AND team_id = $2",
            season_id,
            team_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = standing_row {
            // Calculate total points scored for this team
            let total_points = sqlx::query_scalar!(
                r#"
                SELECT COALESCE(
                    (SELECT SUM(home_score) FROM games WHERE season_id = $1 AND home_team_id = $2 AND status = 'finished'),
                    0
                ) + COALESCE(
                    (SELECT SUM(away_score) FROM games WHERE season_id = $1 AND away_team_id = $2 AND status = 'finished'),
                    0
                ) as "total!"
                "#,
                season_id,
                team_id
            )
            .fetch_one(&self.pool)
            .await?;

            Ok(Some(LeagueStanding {
                id: row.id,
                season_id: row.season_id,
                team_id: row.team_id,
                games_played: row.games_played,
                wins: row.wins,
                draws: row.draws,
                losses: row.losses,
                points: row.points,
                position: row.position,
                last_updated: row.last_updated,
                total_points_scored: Some(total_points as i32),
            }))
        } else {
            Ok(None)
        }
    }
}