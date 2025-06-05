use chrono::{DateTime, Utc, Duration};
use sqlx::PgPool;
use uuid::Uuid;
use crate::models::league::*;

/// Service responsible for season management
#[derive(Debug, Clone)]
pub struct SeasonService {
    pool: PgPool,
}

impl SeasonService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new league season
    pub async fn create_season(&self, request: CreateSeasonRequest) -> Result<LeagueSeason, sqlx::Error> {
        // Calculate end date based on number of teams
        let team_count = request.team_ids.len();
        let total_weeks = (team_count - 1) * 2; // Each team plays every other team twice
        let end_date = request.start_date + Duration::weeks(total_weeks as i64);

        // Mark any existing active seasons as inactive
        sqlx::query!(
            "UPDATE league_seasons SET is_active = FALSE WHERE is_active = TRUE"
        )
        .execute(&self.pool)
        .await?;

        // Create new season
        let season = sqlx::query_as!(
            LeagueSeason,
            r#"
            INSERT INTO league_seasons (name, start_date, end_date, is_active)
            VALUES ($1, $2, $3, TRUE)
            RETURNING *
            "#,
            request.name,
            request.start_date,
            end_date
        )
        .fetch_one(&self.pool)
        .await?;

        tracing::info!(
            "Created new season '{}' with {} teams, running from {} to {}",
            season.name,
            team_count,
            season.start_date,
            season.end_date
        );

        Ok(season)
    }

    /// Get a season by ID
    pub async fn get_season(&self, season_id: Uuid) -> Result<Option<LeagueSeason>, sqlx::Error> {
        sqlx::query_as!(
            LeagueSeason,
            "SELECT * FROM league_seasons WHERE id = $1",
            season_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Get the currently active season
    pub async fn get_active_season(&self) -> Result<Option<LeagueSeason>, sqlx::Error> {
        sqlx::query_as!(
            LeagueSeason,
            "SELECT * FROM league_seasons WHERE is_active = TRUE ORDER BY created_at DESC LIMIT 1"
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Get all seasons, ordered by most recent first
    pub async fn get_all_seasons(&self, limit: Option<i64>) -> Result<Vec<LeagueSeason>, sqlx::Error> {
        let limit = limit.unwrap_or(50);
        
        sqlx::query_as!(
            LeagueSeason,
            "SELECT * FROM league_seasons ORDER BY created_at DESC LIMIT $1",
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Update season details
    pub async fn update_season(
        &self,
        season_id: Uuid,
        name: Option<String>,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
        is_active: Option<bool>,
    ) -> Result<LeagueSeason, sqlx::Error> {
        // Build dynamic update query
        let mut query = "UPDATE league_seasons SET updated_at = NOW()".to_string();
        let mut param_count = 1;
        let mut params: Vec<Box<dyn sqlx::Encode<sqlx::Postgres> + Send + Sync>> = Vec::new();

        if let Some(ref name) = name {
            query.push_str(&format!(", name = ${}", param_count));
            params.push(Box::new(name.clone()));
            param_count += 1;
        }

        if let Some(start_date) = start_date {
            query.push_str(&format!(", start_date = ${}", param_count));
            params.push(Box::new(start_date));
            param_count += 1;
        }

        if let Some(end_date) = end_date {
            query.push_str(&format!(", end_date = ${}", param_count));
            params.push(Box::new(end_date));
            param_count += 1;
        }

        if let Some(is_active) = is_active {
            // If setting this season to active, deactivate others first
            if is_active {
                sqlx::query!("UPDATE league_seasons SET is_active = FALSE WHERE is_active = TRUE")
                    .execute(&self.pool)
                    .await?;
            }
            
            query.push_str(&format!(", is_active = ${}", param_count));
            params.push(Box::new(is_active));
            param_count += 1;
        }

        query.push_str(&format!(" WHERE id = ${} RETURNING *", param_count));

        // For now, let's use a simpler approach with individual field updates
        let updated_season = if name.is_some() || start_date.is_some() || end_date.is_some() || is_active.is_some() {
            sqlx::query_as!(
                LeagueSeason,
                r#"
                UPDATE league_seasons 
                SET name = COALESCE($1, name),
                    start_date = COALESCE($2, start_date),
                    end_date = COALESCE($3, end_date),
                    is_active = COALESCE($4, is_active),
                    updated_at = NOW()
                WHERE id = $5
                RETURNING *
                "#,
                name,
                start_date,
                end_date,
                is_active,
                season_id
            )
            .fetch_one(&self.pool)
            .await?
        } else {
            // No changes, just return current season
            self.get_season(season_id).await?.ok_or(sqlx::Error::RowNotFound)?
        };

        Ok(updated_season)
    }

    /// Delete a season (and all associated data)
    pub async fn delete_season(&self, season_id: Uuid) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // Delete in correct order due to foreign key constraints
        sqlx::query!("DELETE FROM league_standings WHERE season_id = $1", season_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query!("DELETE FROM league_games WHERE season_id = $1", season_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query!("DELETE FROM league_seasons WHERE id = $1", season_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        tracing::info!("Deleted season {}", season_id);
        Ok(())
    }

    /// Get season statistics
    pub async fn get_season_statistics(&self, season_id: Uuid) -> Result<SeasonStatistics, sqlx::Error> {
        let season = self.get_season(season_id).await?
            .ok_or(sqlx::Error::RowNotFound)?;

        let stats = sqlx::query!(
            r#"
            SELECT 
                COUNT(DISTINCT lg.id) as total_games,
                SUM(CASE WHEN lg.status = 'finished' THEN 1 ELSE 0 END) as completed_games,
                COUNT(DISTINCT ls.team_id) as total_teams,
                MAX(lg.week_number) as total_weeks
            FROM league_games lg
            LEFT JOIN league_standings ls ON ls.season_id = lg.season_id
            WHERE lg.season_id = $1
            "#,
            season_id
        )
        .fetch_one(&self.pool)
        .await?;

        let progress_percentage = if stats.total_games.unwrap_or(0) > 0 {
            (stats.completed_games.unwrap_or(0) as f32 / stats.total_games.unwrap_or(1) as f32) * 100.0
        } else {
            0.0
        };

        Ok(SeasonStatistics {
            season: season.clone(),
            total_games: stats.total_games.unwrap_or(0) as i32,
            completed_games: stats.completed_games.unwrap_or(0) as i32,
            total_teams: stats.total_teams.unwrap_or(0) as i32,
            total_weeks: stats.total_weeks.unwrap_or(0),
            progress_percentage,
            started: chrono::Utc::now() >= season.start_date,
            finished: chrono::Utc::now() >= season.end_date,
        })
    }

    /// Get seasons within a date range
    pub async fn get_seasons_in_range(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<LeagueSeason>, sqlx::Error> {
        sqlx::query_as!(
            LeagueSeason,
            r#"
            SELECT * FROM league_seasons 
            WHERE start_date <= $2 AND end_date >= $1
            ORDER BY start_date DESC
            "#,
            start_date,
            end_date
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Check if a season name already exists
    pub async fn season_name_exists(&self, name: &str) -> Result<bool, sqlx::Error> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM league_seasons WHERE LOWER(name) = LOWER($1)",
            name
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count.unwrap_or(0) > 0)
    }

    /// Archive old seasons (mark as inactive and cleanup)
    pub async fn archive_old_seasons(&self, cutoff_date: DateTime<Utc>) -> Result<i32, sqlx::Error> {
        let updated_count = sqlx::query!(
            r#"
            UPDATE league_seasons 
            SET is_active = FALSE, updated_at = NOW()
            WHERE end_date < $1 AND is_active = TRUE
            "#,
            cutoff_date
        )
        .execute(&self.pool)
        .await?
        .rows_affected();

        tracing::info!("Archived {} old seasons", updated_count);
        Ok(updated_count as i32)
    }
}

/// Season statistics and metadata
#[derive(Debug, Clone)]
pub struct SeasonStatistics {
    pub season: LeagueSeason,
    pub total_games: i32,
    pub completed_games: i32,
    pub total_teams: i32,
    pub total_weeks: i32,
    pub progress_percentage: f32,
    pub started: bool,
    pub finished: bool,
}