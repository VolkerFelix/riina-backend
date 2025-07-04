use uuid::Uuid;
use sqlx::PgPool;
use crate::models::common::MatchResult;
use chrono::{DateTime, Utc};

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
    pub average_power: u32,
}

#[derive(Debug, Clone)]
pub struct TeamSnapshot {
    pub team_id: Uuid,
    pub total_stamina: i32,
    pub total_strength: i32,
    pub member_count: i32,
    pub snapshot_time: DateTime<Utc>,
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
                let average_power = if member_count > 0 { total_power / member_count } else { 0 };
                Ok(TeamPower {
                    team_id: team_stats.team_id,
                    average_power,
                })
            }
            None => {
                Ok(TeamPower {
                    team_id: *team_id,
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


    /// Take a snapshot of team's current total stats and store it in the database
    pub async fn take_team_snapshot(
        pool: &PgPool,
        game_id: &Uuid,
        team_id: &Uuid,
        snapshot_type: &str, // "start" or "end"
    ) -> Result<TeamSnapshot, sqlx::Error> {
        let snapshot_time = Utc::now();
        
        // Get current team totals from user_avatars
        let team_stats = sqlx::query!(
            r#"
            SELECT
                COUNT(tm.user_id) as member_count,
                COALESCE(SUM(ua.stamina), 0) as total_stamina,
                COALESCE(SUM(ua.strength), 0) as total_strength
            FROM team_members tm
            LEFT JOIN user_avatars ua ON tm.user_id = ua.user_id
            WHERE tm.team_id = $1
            AND tm.status = 'active'
            "#,
            team_id
        )
        .fetch_one(pool)
        .await?;

        let total_stamina = team_stats.total_stamina.unwrap_or(0) as i32;
        let total_strength = team_stats.total_strength.unwrap_or(0) as i32;
        let member_count = team_stats.member_count.unwrap_or(0) as i32;

        // Store snapshot in database
        sqlx::query!(
            r#"
            INSERT INTO game_team_snapshots 
            (game_id, team_id, snapshot_type, total_stamina, total_strength, member_count, snapshot_time)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (game_id, team_id, snapshot_type) 
            DO UPDATE SET 
                total_stamina = EXCLUDED.total_stamina,
                total_strength = EXCLUDED.total_strength,
                member_count = EXCLUDED.member_count,
                snapshot_time = EXCLUDED.snapshot_time
            "#,
            game_id,
            team_id,
            snapshot_type,
            total_stamina,
            total_strength,
            member_count,
            snapshot_time
        )
        .execute(pool)
        .await?;

        tracing::info!("📸 Took {} snapshot for team {} in game {}: stamina={}, strength={}, members={}", 
            snapshot_type, team_id, game_id, total_stamina, total_strength, member_count);

        Ok(TeamSnapshot {
            team_id: *team_id,
            total_stamina,
            total_strength,
            member_count,
            snapshot_time,
        })
    }

    /// Get a team's snapshot for a specific game and snapshot type
    pub async fn get_team_snapshot(
        pool: &PgPool,
        game_id: &Uuid,
        team_id: &Uuid,
        snapshot_type: &str,
    ) -> Result<Option<TeamSnapshot>, sqlx::Error> {
        let snapshot = sqlx::query!(
            r#"
            SELECT total_stamina, total_strength, member_count, snapshot_time
            FROM game_team_snapshots
            WHERE game_id = $1 AND team_id = $2 AND snapshot_type = $3
            "#,
            game_id,
            team_id,
            snapshot_type
        )
        .fetch_optional(pool)
        .await?;

        Ok(snapshot.map(|s| TeamSnapshot {
            team_id: *team_id,
            total_stamina: s.total_stamina,
            total_strength: s.total_strength,
            member_count: s.member_count,
            snapshot_time: s.snapshot_time,
        }))
    }

    /// Evaluate game based on snapshots (difference between end and start)
    pub async fn evaluate_game_with_snapshots(
        pool: &PgPool,
        game_id: &Uuid,
        home_team_id: &Uuid,
        away_team_id: &Uuid,
    ) -> Result<GameStats, sqlx::Error> {
        tracing::info!("🎮 Evaluating game {} with snapshots: {} vs {}", game_id, home_team_id, away_team_id);

        // Get start and end snapshots for both teams
        let home_start = Self::get_team_snapshot(pool, game_id, home_team_id, "start").await?;
        let home_end = Self::get_team_snapshot(pool, game_id, home_team_id, "end").await?;
        let away_start = Self::get_team_snapshot(pool, game_id, away_team_id, "start").await?;
        let away_end = Self::get_team_snapshot(pool, game_id, away_team_id, "end").await?;

        // Calculate score differences
        let home_score = match (home_start, home_end) {
            (Some(start), Some(end)) => {
                let stamina_diff = end.total_stamina - start.total_stamina;
                let strength_diff = end.total_strength - start.total_strength;
                std::cmp::max(0, stamina_diff + strength_diff) as u32
            }
            _ => {
                tracing::warn!("Missing snapshots for home team {} in game {}", home_team_id, game_id);
                0
            }
        };

        let away_score = match (away_start, away_end) {
            (Some(start), Some(end)) => {
                let stamina_diff = end.total_stamina - start.total_stamina;
                let strength_diff = end.total_strength - start.total_strength;
                std::cmp::max(0, stamina_diff + strength_diff) as u32
            }
            _ => {
                tracing::warn!("Missing snapshots for away team {} in game {}", away_team_id, game_id);
                0
            }
        };

        // Determine winner based on score differences
        let (home_result, away_result, winner_team_id) = if home_score > away_score {
            (MatchResult::Win, MatchResult::Loss, Some(*home_team_id))
        } else if home_score < away_score {
            (MatchResult::Loss, MatchResult::Win, Some(*away_team_id))
        } else {
            (MatchResult::Draw, MatchResult::Draw, None)
        };

        tracing::info!("Game {} result: Home {} ({}), Away {} ({})", 
            game_id, home_score, home_result, away_score, away_result);

        Ok(GameStats {
            game_id: *game_id,
            home_team_name: String::new(),
            away_team_name: String::new(),
            home_team_score: home_score,
            away_team_score: away_score,
            home_team_result: home_result,
            away_team_result: away_result,
            winner_team_id,
            home_score,
            away_score,
        })
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