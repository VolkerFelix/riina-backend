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
    workout_count: i32,
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

        // Get profile picture URLs for MVP and LVP
        let mvp_profile_picture_url = if let Some(ref mvp_player) = mvp {
            self.get_user_profile_picture_url(mvp_player.user_id).await.unwrap_or(None)
        } else {
            None
        };

        let lvp_profile_picture_url = if let Some(ref lvp_player) = lvp {
            self.get_user_profile_picture_url(lvp_player.user_id).await.unwrap_or(None)
        } else {
            None
        };

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
                mvp_profile_picture_url,
                lvp_user_id,
                lvp_username,
                lvp_team_id,
                lvp_score_contribution,
                lvp_profile_picture_url,
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
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31)
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
            mvp_profile_picture_url.as_deref(),
            lvp.as_ref().map(|p| p.user_id),
            lvp.as_ref().map(|p| p.username.clone()),
            lvp.as_ref().map(|p| p.team_id),
            lvp.as_ref().map(|p| p.total_score),
            lvp_profile_picture_url.as_deref(),
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
    /// This includes ALL team members, even those without score events (they get 0 points)
    async fn get_player_contributions(&self, game_id: Uuid) -> Result<Vec<PlayerContribution>, sqlx::Error> {
        // First, get the game's team information
        let game_info = sqlx::query!(
            r#"
            SELECT home_team_id, away_team_id
            FROM games
            WHERE id = $1
            "#,
            game_id
        )
        .fetch_one(&self.pool)
        .await?;

        // Get all team members for both teams
        let all_team_members = sqlx::query!(
            r#"
            SELECT 
                tm.user_id,
                u.username,
                tm.team_id,
                CASE 
                    WHEN tm.team_id = $1 THEN 'home'
                    WHEN tm.team_id = $2 THEN 'away'
                END as team_side
            FROM team_members tm
            JOIN users u ON tm.user_id = u.id
            WHERE tm.team_id IN ($1, $2) AND tm.status = 'active'
            "#,
            game_info.home_team_id,
            game_info.away_team_id
        )
        .fetch_all(&self.pool)
        .await?;

        // Get score contributions and workout counts for players who have them
        let score_contributions = sqlx::query!(
            r#"
            SELECT
                user_id,
                SUM(score_points) as total_score,
                COUNT(*) as workout_count
            FROM live_score_events
            WHERE game_id = $1
            GROUP BY user_id
            "#,
            game_id
        )
        .fetch_all(&self.pool)
        .await?;

        // Create maps for quick lookup
        let score_map: std::collections::HashMap<Uuid, i32> = score_contributions
            .iter()
            .map(|row| (row.user_id, row.total_score.unwrap_or(0.0) as i32))
            .collect();
        
        let workout_count_map: std::collections::HashMap<Uuid, i32> = score_contributions
            .iter()
            .map(|row| (row.user_id, row.workout_count.unwrap_or(0) as i32))
            .collect();

        // Build the final contributions list with all team members
        let contributions = all_team_members
            .into_iter()
            .map(|member| PlayerContribution {
                user_id: member.user_id,
                username: member.username,
                team_id: member.team_id,
                team_side: member.team_side.unwrap_or_else(|| "unknown".to_string()),
                total_score: score_map.get(&member.user_id).copied().unwrap_or(0),
                workout_count: workout_count_map.get(&member.user_id).copied().unwrap_or(0),
            })
            .collect();

        Ok(contributions)
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
    /// Uses all players for team score calculation
    fn calculate_single_team_stats(&self, players: &[&PlayerContribution]) -> TeamStats {
        if players.is_empty() {
            return TeamStats {
                avg_score_per_player: Some(0.0),
                total_workouts: 0,
                top_scorer: None,
                lowest_performer: None,
            };
        }

        // Sort players by score (descending) to get best performers
        let mut sorted_players = players.to_vec();
        sorted_players.sort_by(|a, b| b.total_score.cmp(&a.total_score));

        // Count all players for team score
        let total_score: i32 = sorted_players.iter().map(|p| p.total_score).sum();
        let contributing_count = sorted_players.len();
        let avg_score = if contributing_count > 0 {
            total_score as f32 / contributing_count as f32
        } else {
            0.0
        };

        // Get total workouts (sum of all workout events from all players)
        let total_workouts = players.iter().map(|p| p.workout_count).sum::<i32>();

        // Find top scorer (across all players)
        let top_scorer = sorted_players.first().map(|&p| p.clone());

        // Find lowest performer (across all players)
        let lowest_performer = sorted_players.last().map(|&p| p.clone());

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

        // Use all players for MVP/LVP calculation
        let eligible_players = contributions;

        // If only one player participated, they can only be MVP, not LVP
        if eligible_players.len() == 1 {
            let mvp = eligible_players[0].clone();
            return (Some(mvp), None);
        }

        // Find MVP among all players
        let mvp = eligible_players
            .iter()
            .max_by_key(|p| p.total_score)
            .cloned();

        // Find LVP among all players who are NOT the MVP
        let lvp = if let Some(mvp_player) = &mvp {
            eligible_players
                .iter()
                .filter(|p| p.user_id != mvp_player.user_id)
                .min_by_key(|p| p.total_score)
                .cloned()
        } else {
            None
        };

        (mvp, lvp)
    }

    /// Get a game summary by game ID
    pub async fn get_game_summary(&self, game_id: Uuid) -> Result<Option<GameSummary>, sqlx::Error> {
        let summary = sqlx::query_as!(
            GameSummary,
            r#"
            SELECT 
                id, game_id, final_home_score, final_away_score, game_start_date, game_end_date,
                mvp_user_id, mvp_username, mvp_team_id, mvp_score_contribution, mvp_profile_picture_url,
                lvp_user_id, lvp_username, lvp_team_id, lvp_score_contribution, lvp_profile_picture_url,
                home_team_avg_score_per_player, home_team_total_workouts, home_team_top_scorer_id, 
                home_team_top_scorer_username, home_team_top_scorer_points, home_team_lowest_performer_id, 
                home_team_lowest_performer_username, home_team_lowest_performer_points,
                away_team_avg_score_per_player, away_team_total_workouts, away_team_top_scorer_id, 
                away_team_top_scorer_username, away_team_top_scorer_points, away_team_lowest_performer_id, 
                away_team_lowest_performer_username, away_team_lowest_performer_points,
                created_at, updated_at
            FROM game_summaries
            WHERE game_id = $1
            "#,
            game_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(summary)
    }

    /// Get user profile picture URL by user ID
    async fn get_user_profile_picture_url(&self, user_id: Uuid) -> Result<Option<String>, sqlx::Error> {
        let result = sqlx::query!(
            "SELECT profile_picture_url FROM users WHERE id = $1",
            user_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.and_then(|row| row.profile_picture_url))
    }
}

#[derive(Debug)]
struct TeamStats {
    avg_score_per_player: Option<f32>,
    total_workouts: i32,
    top_scorer: Option<PlayerContribution>,
    lowest_performer: Option<PlayerContribution>,
}
