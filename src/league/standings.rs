use sqlx::PgPool;
use uuid::Uuid;
use crate::models::league::*;
use crate::utils::team_power;

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
        let home_result = sqlx::query!(
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
        let away_result = sqlx::query!(
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
            ORDER BY ls.points DESC, (ls.wins * 3 + ls.draws) DESC, ls.wins DESC
            "#,
            season_id
        )
        .fetch_all(&self.pool)
        .await?;


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
                    },
                    team_name: row.team_name,
                    team_color: row.team_color,
                    recent_form: vec!['W', 'L', 'D'], // TODO: Calculate actual form
                    team_power: team_powers.get(&row.team_id).copied().unwrap_or(0),
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

    /// Recalculate all positions based on current points
    async fn recalculate_positions_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        season_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        // Get all standings ordered by points/wins
        let standings = sqlx::query!(
            r#"
            SELECT team_id
            FROM league_standings 
            WHERE season_id = $1
            ORDER BY points DESC, wins DESC, (wins * 3 + draws) DESC
            "#,
            season_id
        )
        .fetch_all(&mut **tx)
        .await?;

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
        sqlx::query_as!(
            LeagueStanding,
            "SELECT * FROM league_standings WHERE season_id = $1 AND team_id = $2",
            season_id,
            team_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Get top N teams
    pub async fn get_top_teams(
        &self,
        season_id: Uuid,
        limit: i64,
    ) -> Result<Vec<StandingWithTeam>, sqlx::Error> {
        let standings_with_teams = sqlx::query!(
            r#"
            SELECT 
                ls.*,
                t.team_name,
                t.team_color
            FROM league_standings ls
            JOIN teams t ON ls.team_id = t.id
            WHERE ls.season_id = $1
            ORDER BY ls.points DESC, ls.wins DESC
            LIMIT $2
            "#,
            season_id,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        // Calculate team powers
        let team_ids: Vec<Uuid> = standings_with_teams.iter().map(|row| row.team_id).collect();
        let team_powers = team_power::calculate_multiple_team_powers(&team_ids, &self.pool).await?;

        Ok(standings_with_teams
            .into_iter()
            .map(|row| StandingWithTeam {
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
                },
                team_name: row.team_name,
                team_color: row.team_color,
                recent_form: vec!['W', 'L', 'D'], // TODO: Calculate actual form
                team_power: team_powers.get(&row.team_id).copied().unwrap_or(0),
            })
            .collect())
    }
}