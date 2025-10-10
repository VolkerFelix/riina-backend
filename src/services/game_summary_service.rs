use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;

use crate::models::league::{GameSummary, LeagueGame};

#[derive(Debug)]
pub struct GameSummaryService {
    pool: PgPool,
}

#[derive(Debug, Clone)]
struct PlayerContribution {
    user_id: Uuid,
    username: String,
    team_id: Uuid,
    team_side: String, // 'home' or 'away'
    total_score: i32,
}

impl GameSummaryService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Calculate and create a game summary for a finished game
    pub async fn create_game_summary(&self, game: &LeagueGame) -> Result<GameSummary, sqlx::Error> {
        tracing::info!("📊 Creating game summary for game {}", game.id);

        // Get player contributions from live_score_events
        let player_contributions = self.get_player_contributions(game.id).await?;

        // Calculate team statistics
        let (home_stats, away_stats) = self.calculate_team_statistics(&player_contributions);

        // Determine MVP and LVP across both teams
        let (mvp, lvp) = self.calculate_mvp_lvp(&player_contributions);

        // Get game start and end times
        let game_start_date = game.game_start_time.unwrap_or(game.created_at);
        let game_end_date = game.game_end_time.unwrap_or(Utc::now());

        // Insert the game summary into the database
        let summary = sqlx::query_as!(
            GameSummary,
            r#"
            INSERT INTO game_summaries (
                game_id,
                final_home_score,
                final_away_score,
                game_start_date,
                game_end_date,
                mvp_user_id,
                mvp_username,
                mvp_team_id,
                mvp_score_contribution,
                lvp_user_id,
                lvp_username,
                lvp_team_id,
                lvp_score_contribution,
                home_team_avg_score_per_player,
                home_team_total_workouts,
                home_team_top_scorer_id,
                home_team_top_scorer_username,
                home_team_top_scorer_points,
                home_team_lowest_performer_id,
                home_team_lowest_performer_username,
                home_team_lowest_performer_points,
                away_team_avg_score_per_player,
                away_team_total_workouts,
                away_team_top_scorer_id,
                away_team_top_scorer_username,
                away_team_top_scorer_points,
                away_team_lowest_performer_id,
                away_team_lowest_performer_username,
                away_team_lowest_performer_points
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29)
            RETURNING *
            "#,
            game.id,
            game.home_score,
            game.away_score,
            game_start_date,
            game_end_date,
            mvp.as_ref().map(|p| p.user_id),
            mvp.as_ref().map(|p| p.username.clone()),
            mvp.as_ref().map(|p| p.team_id),
            mvp.as_ref().map(|p| p.total_score),
            lvp.as_ref().map(|p| p.user_id),
            lvp.as_ref().map(|p| p.username.clone()),
            lvp.as_ref().map(|p| p.team_id),
            lvp.as_ref().map(|p| p.total_score),
            home_stats.avg_score_per_player,
            home_stats.total_workouts,
            home_stats.top_scorer.as_ref().map(|p| p.user_id),
            home_stats.top_scorer.as_ref().map(|p| p.username.clone()),
            home_stats.top_scorer.as_ref().map(|p| p.total_score),
            home_stats.lowest_performer.as_ref().map(|p| p.user_id),
            home_stats.lowest_performer.as_ref().map(|p| p.username.clone()),
            home_stats.lowest_performer.as_ref().map(|p| p.total_score),
            away_stats.avg_score_per_player,
            away_stats.total_workouts,
            away_stats.top_scorer.as_ref().map(|p| p.user_id),
            away_stats.top_scorer.as_ref().map(|p| p.username.clone()),
            away_stats.top_scorer.as_ref().map(|p| p.total_score),
            away_stats.lowest_performer.as_ref().map(|p| p.user_id),
            away_stats.lowest_performer.as_ref().map(|p| p.username.clone()),
            away_stats.lowest_performer.as_ref().map(|p| p.total_score),
        )
        .fetch_one(&self.pool)
        .await?;

        tracing::info!("✅ Game summary created successfully for game {}", game.id);
        Ok(summary)
    }

    /// Get player contributions from live_score_events table
    async fn get_player_contributions(&self, game_id: Uuid) -> Result<Vec<PlayerContribution>, sqlx::Error> {
        let contributions = sqlx::query!(
            r#"
            SELECT
                user_id,
                username,
                team_id,
                team_side,
                SUM(score_points) as total_score
            FROM live_score_events
            WHERE game_id = $1
            GROUP BY user_id, username, team_id, team_side
            ORDER BY total_score DESC
            "#,
            game_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(contributions
            .into_iter()
            .map(|row| PlayerContribution {
                user_id: row.user_id,
                username: row.username,
                team_id: row.team_id,
                team_side: row.team_side,
                total_score: row.total_score.unwrap_or(0.0) as i32,
            })
            .collect())
    }

    /// Calculate team statistics (avg score, top scorer, lowest performer, etc.)
    fn calculate_team_statistics(
        &self,
        contributions: &[PlayerContribution],
    ) -> (TeamStats, TeamStats) {
        let home_players: Vec<_> = contributions
            .iter()
            .filter(|p| p.team_side == "home")
            .collect();

        let away_players: Vec<_> = contributions
            .iter()
            .filter(|p| p.team_side == "away")
            .collect();

        let home_stats = self.calculate_single_team_stats(&home_players);
        let away_stats = self.calculate_single_team_stats(&away_players);

        (home_stats, away_stats)
    }

    /// Calculate statistics for a single team
    fn calculate_single_team_stats(&self, players: &[&PlayerContribution]) -> TeamStats {
        if players.is_empty() {
            return TeamStats {
                avg_score_per_player: Some(0.0),
                total_workouts: 0,
                top_scorer: None,
                lowest_performer: None,
            };
        }

        let total_score: i32 = players.iter().map(|p| p.total_score).sum();
        let avg_score = total_score as f32 / players.len() as f32;

        // Get total workouts (count of unique workout events)
        let total_workouts = players.len() as i32;

        // Find top scorer
        let top_scorer = players
            .iter()
            .max_by_key(|p| p.total_score)
            .map(|&p| p.clone());

        // Find lowest performer
        let lowest_performer = players
            .iter()
            .min_by_key(|p| p.total_score)
            .map(|&p| p.clone());

        TeamStats {
            avg_score_per_player: Some(avg_score),
            total_workouts,
            top_scorer,
            lowest_performer,
        }
    }

    /// Calculate MVP (Most Valuable Player) and LVP (Least Valuable Player) across both teams
    fn calculate_mvp_lvp(
        &self,
        contributions: &[PlayerContribution],
    ) -> (Option<PlayerContribution>, Option<PlayerContribution>) {
        if contributions.is_empty() {
            return (None, None);
        }

        let mvp = contributions
            .iter()
            .max_by_key(|p| p.total_score)
            .map(|p| p.clone());

        let lvp = contributions
            .iter()
            .min_by_key(|p| p.total_score)
            .map(|p| p.clone());

        (mvp, lvp)
    }

    /// Get a game summary by game ID
    pub async fn get_game_summary(&self, game_id: Uuid) -> Result<Option<GameSummary>, sqlx::Error> {
        let summary = sqlx::query_as!(
            GameSummary,
            r#"
            SELECT * FROM game_summaries
            WHERE game_id = $1
            "#,
            game_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(summary)
    }
}

#[derive(Debug)]
struct TeamStats {
    avg_score_per_player: Option<f32>,
    total_workouts: i32,
    top_scorer: Option<PlayerContribution>,
    lowest_performer: Option<PlayerContribution>,
}
