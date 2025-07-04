use uuid::Uuid;
use sqlx::PgPool;
use crate::models::common::MatchResult;

// Using MatchResult from common module instead of duplicate GameResult enum

#[derive(Debug, Clone)]
pub struct GameStats {
    pub game_id: Uuid,
    pub home_team_name: String,
    pub away_team_name: String,
    pub home_team_score: u32,
    pub away_team_score: u32,
    pub home_team_result: MatchResult,
    pub away_team_result: MatchResult,
    pub winner_team_id: Option<Uuid>,
    pub home_score: u32,  // Alias for compatibility
    pub away_score: u32,  // Alias for compatibility
}

struct TeamPower {
    pub team_id: Uuid,
    pub total_power: u32,
    pub member_count: u32,
    pub average_power: u32,
}

pub struct GameEvaluator;

impl GameEvaluator {
    pub async fn evaluate_game(pool: &PgPool, home_team_id: &Uuid, away_team_id: &Uuid) -> Result<GameStats, sqlx::Error> {
        tracing::info!("🎮 Evaluating game: {} vs {}", home_team_id, away_team_id);

        let home_team_power = Self::calc_team_power(pool, home_team_id).await?;
        let away_team_power = Self::calc_team_power(pool, away_team_id).await?;

        tracing::info!("Team Powers: Home {} ({}), Away {} ({})",
            home_team_power.team_id, home_team_power.average_power,
            away_team_power.team_id, away_team_power.average_power
        );

        let game_stats = Self::calc_game_outcome(&home_team_power, &away_team_power);

        tracing::info!("Game Stats: Home {} ({}), Away {} ({})",
            game_stats.home_team_score, game_stats.home_team_result,
            game_stats.away_team_score, game_stats.away_team_result
        );

        Ok(game_stats)
    }

    async fn calc_team_power(pool: &PgPool, team_id: &Uuid) -> Result<TeamPower, sqlx::Error> {
        let team_stats = sqlx::query!(
            r#"
            SELECT
                tm.team_id,
                COUNT(tm.user_id) as member_count,
                COALESCE(SUM(ua.stamina + ua.strength), 0) as total_power
            FROM team_members tm
            LEFT JOIN user_avatars ua ON tm.user_id = ua.user_id
            WHERE tm.team_id = $1
            AND tm.status = 'active'
            GROUP BY tm.team_id
            "#,
            team_id
        )
        .fetch_optional(pool)
        .await?;

        match team_stats {
            Some(team_stats) => {
                let total_power = team_stats.total_power.unwrap_or(0) as u32;
                let member_count = team_stats.member_count.unwrap_or(0) as u32;
                let average_power = if member_count > 0 { (total_power / member_count) as u32 } else { 0 };
                Ok(TeamPower {
                    team_id: team_stats.team_id,
                    member_count,
                    total_power,
                    average_power,
                })
            }
            None => {
                Ok(TeamPower {
                    team_id: team_id.clone(),
                    member_count: 0,
                    total_power: 0,
                    average_power: 0,
                })
            }
        }
    }

    fn calc_game_outcome(home_team_power: &TeamPower, away_team_power: &TeamPower) -> GameStats {
        let home_team_score = home_team_power.average_power;
        let away_team_score = away_team_power.average_power;
        let home_team_result;
        let away_team_result;
        let mut winner_team_id = None;

        if home_team_score > away_team_score {
            home_team_result = MatchResult::Win;
            away_team_result = MatchResult::Loss;
            winner_team_id = Some(home_team_power.team_id);
        } else if home_team_score < away_team_score {
            home_team_result = MatchResult::Loss;
            away_team_result = MatchResult::Win;
            winner_team_id = Some(away_team_power.team_id);
        } else {
            home_team_result = MatchResult::Draw;
            away_team_result = MatchResult::Draw;
        }

        GameStats {
            game_id: Uuid::nil(), // Will be set by caller if needed
            home_team_name: String::new(), // Will be set by caller if needed
            away_team_name: String::new(), // Will be set by caller if needed
            home_team_score,
            away_team_score,
            home_team_result,
            away_team_result,
            winner_team_id,
            home_score: home_team_score,  // Alias for compatibility
            away_score: away_team_score,  // Alias for compatibility
        }
    }

    pub async fn evaluate_todays_games(pool: &PgPool) -> Result<Vec<(Uuid, GameStats)>, sqlx::Error> {
        // Get all games for today
        let pending_games = sqlx::query!(
            r#"
            SELECT
                lg.id as game_id,
                lg.home_team_id,
                lg.away_team_id,
                lg.scheduled_time
            FROM league_games lg
            WHERE DATE(lg.scheduled_time) = CURRENT_DATE
            AND lg.status = 'scheduled'
            ORDER BY lg.scheduled_time
            "#
        )
        .fetch_all(pool)
        .await?;

        let mut results = Vec::new();

        for game in pending_games {
            tracing::info!("🎯 Evaluating game {} for today", game.game_id);
            let game_stats = Self::evaluate_game(pool, &game.home_team_id, &game.away_team_id).await?;
            results.push((game.game_id, game_stats));
        }

        Ok(results)
    }

    /// Get live scores for all active week-long games
    pub async fn get_live_scores_for_active_games(pool: &PgPool) -> Result<Vec<(Uuid, GameStats)>, sqlx::Error> {
        // Get all games that are currently in progress (within their week window)
        let active_games = sqlx::query!(
            r#"
            SELECT
                lg.id as game_id,
                lg.home_team_id,
                lg.away_team_id,
                lg.week_start_date,
                lg.week_end_date
            FROM league_games lg
            WHERE lg.status = 'in_progress'
            AND CURRENT_DATE BETWEEN lg.week_start_date AND lg.week_end_date
            ORDER BY lg.week_number
            "#
        )
        .fetch_all(pool)
        .await?;

        let mut results = Vec::new();

        for game in active_games {
            tracing::debug!("📊 Calculating live score for game {}", game.game_id);
            let game_stats = Self::calculate_live_score(pool, &game.home_team_id, &game.away_team_id, game.week_start_date, game.week_end_date).await?;
            results.push((game.game_id, game_stats));
        }

        Ok(results)
    }

    /// Calculate live score for a week-long game based on health data within the week
    pub async fn calculate_live_score(
        pool: &PgPool, 
        home_team_id: &Uuid, 
        away_team_id: &Uuid,
        week_start: Option<chrono::NaiveDate>,
        week_end: Option<chrono::NaiveDate>
    ) -> Result<GameStats, sqlx::Error> {
        let week_start = week_start.unwrap_or_else(|| chrono::Utc::now().date_naive());
        let week_end = week_end.unwrap_or_else(|| chrono::Utc::now().date_naive());

        let home_team_score = Self::calculate_team_week_score(pool, home_team_id, week_start, week_end).await?;
        let away_team_score = Self::calculate_team_week_score(pool, away_team_id, week_start, week_end).await?;

        // Determine winner based on accumulated points
        let (home_result, away_result, winner_team_id) = if home_team_score > away_team_score {
            (MatchResult::Win, MatchResult::Loss, Some(*home_team_id))
        } else if home_team_score < away_team_score {
            (MatchResult::Loss, MatchResult::Win, Some(*away_team_id))
        } else {
            (MatchResult::Draw, MatchResult::Draw, None)
        };

        Ok(GameStats {
            game_id: Uuid::nil(),
            home_team_name: String::new(),
            away_team_name: String::new(),
            home_team_score,
            away_team_score,
            home_team_result: home_result,
            away_team_result: away_result,
            winner_team_id,
            home_score: home_team_score,
            away_score: away_team_score,
        })
    }

    /// Calculate team's total score for the week based on health data
    async fn calculate_team_week_score(
        pool: &PgPool,
        team_id: &Uuid,
        week_start: chrono::NaiveDate,
        week_end: chrono::NaiveDate
    ) -> Result<u32, sqlx::Error> {
        let team_score = sqlx::query!(
            r#"
            SELECT 
                COALESCE(SUM(hd.stamina + hd.strength), 0) as total_points
            FROM team_members tm
            JOIN health_data hd ON tm.user_id = hd.user_id
            WHERE tm.team_id = $1
            AND tm.status = 'active'
            AND DATE(hd.upload_date) BETWEEN $2 AND $3
            "#,
            team_id,
            week_start,
            week_end
        )
        .fetch_one(pool)
        .await?;

        Ok(team_score.total_points.unwrap_or(0) as u32)
    }

    pub async fn evaluate_games_for_date(pool: &PgPool, date: chrono::NaiveDate) -> Result<Vec<GameStats>, sqlx::Error> {
        // Get all games for the specified date
        let pending_games = sqlx::query!(
            r#"
            SELECT
                lg.id as game_id,
                lg.home_team_id,
                lg.away_team_id,
                lg.scheduled_time,
                ht.team_name as home_team_name,
                at.team_name as away_team_name
            FROM league_games lg
            JOIN teams ht ON lg.home_team_id = ht.id
            JOIN teams at ON lg.away_team_id = at.id
            WHERE DATE(lg.scheduled_time) = $1
            AND lg.status = 'scheduled'
            ORDER BY lg.scheduled_time
            "#,
            date
        )
        .fetch_all(pool)
        .await?;

        let mut results = Vec::new();

        for game in pending_games {
            tracing::info!("🎯 Evaluating game {} for date {}", game.game_id, date);
            let mut game_stats = Self::evaluate_game(pool, &game.home_team_id, &game.away_team_id).await?;
            
            // Add game ID and team names to the stats
            game_stats.game_id = game.game_id;
            game_stats.home_team_name = game.home_team_name;
            game_stats.away_team_name = game.away_team_name;
            
            results.push(game_stats);
        }

        Ok(results)
    }
}