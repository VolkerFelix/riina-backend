use std::collections::HashSet;
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use crate::models::league::CreateSeasonRequest;

/// Centralized validation service for league operations
pub struct LeagueValidator;

impl LeagueValidator {
    pub fn new() -> Self {
        Self
    }

    /// Validate season creation request
    pub fn validate_create_season_request(&self, request: &CreateSeasonRequest) -> Result<(), sqlx::Error> {
        // Validate season name
        self.validate_season_name(&request.name)?;
        
        // Validate team count and uniqueness
        self.validate_team_ids(&request.team_ids)?;
        
        // Validate start date
        self.validate_start_date(request.start_date)?;
        
        // Validate overall feasibility
        self.validate_season_feasibility(request)?;

        Ok(())
    }

    /// Validate season name
    pub fn validate_season_name(&self, name: &str) -> Result<(), sqlx::Error> {
        let trimmed_name = name.trim();
        
        if trimmed_name.is_empty() {
            return Err(sqlx::Error::Protocol("Season name cannot be empty".into()));
        }

        if trimmed_name.len() > 255 {
            return Err(sqlx::Error::Protocol("Season name too long (maximum 255 characters)".into()));
        }

        // Check for potentially problematic characters
        if trimmed_name.contains('\0') {
            return Err(sqlx::Error::Protocol("Season name contains invalid characters".into()));
        }

        // Ensure name has actual content (not just whitespace/special chars)
        if !trimmed_name.chars().any(|c| c.is_alphanumeric()) {
            return Err(sqlx::Error::Protocol("Season name must contain alphanumeric characters".into()));
        }

        Ok(())
    }

    /// Validate team IDs
    pub fn validate_team_ids(&self, team_ids: &[Uuid]) -> Result<(), sqlx::Error> {
        // Check minimum teams
        if team_ids.len() < 2 {
            return Err(sqlx::Error::Protocol(
                format!("Minimum 2 teams required, got {}", team_ids.len()).into()
            ));
        }

        // Check maximum teams (reasonable limit)
        if team_ids.len() > 20 {
            return Err(sqlx::Error::Protocol(
                format!("Maximum 20 teams allowed, got {}", team_ids.len()).into()
            ));
        }

        // Check for duplicates
        let mut unique_teams = HashSet::new();
        for team_id in team_ids {
            if !unique_teams.insert(team_id) {
                return Err(sqlx::Error::Protocol(
                    format!("Duplicate team ID found: {}", team_id).into()
                ));
            }
        }

        // Validate UUID format (already done by type system, but good to be explicit)
        for team_id in team_ids {
            if team_id.is_nil() {
                return Err(sqlx::Error::Protocol("Nil UUID not allowed for team ID".into()));
            }
        }

        Ok(())
    }

    /// Validate start date
    pub fn validate_start_date(&self, start_date: DateTime<Utc>) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        
        // Allow some tolerance for dates in the near past (e.g., scheduling delays)
        let tolerance = Duration::hours(2);
        if start_date < now - tolerance {
            return Err(sqlx::Error::Protocol(
                format!("Start date {} is too far in the past", start_date).into()
            ));
        }

        // Don't allow dates too far in the future
        let max_future = Duration::days(365); // 1 year
        if start_date > now + max_future {
            return Err(sqlx::Error::Protocol(
                format!("Start date {} is too far in the future (max 1 year)", start_date).into()
            ));
        }

        // Start date validation complete - any future date within 1 year is allowed
        // Games will be scheduled at weekly intervals from the start date
        
        Ok(())
    }

    /// Validate overall season feasibility
    pub fn validate_season_feasibility(&self, request: &CreateSeasonRequest) -> Result<(), sqlx::Error> {
        let team_count = request.team_ids.len();
        
        // Calculate season duration
        let total_weeks = (team_count - 1) * 2;
        let estimated_end_date = request.start_date + Duration::weeks(total_weeks as i64);
        
        // Check if season would be unreasonably long
        let max_reasonable_duration = Duration::weeks(52); // 1 year
        if estimated_end_date - request.start_date > max_reasonable_duration {
            return Err(sqlx::Error::Protocol(
                format!(
                    "Season with {} teams would take {} weeks (over 1 year)",
                    team_count, total_weeks
                ).into()
            ));
        }

        // Check for reasonable minimum duration
        if total_weeks < 2 {
            return Err(sqlx::Error::Protocol(
                "Season too short - need at least 2 weeks".into()
            ));
        }

        Ok(())
    }

    /// Validate game scores
    pub fn validate_game_scores(&self, home_score: i32, away_score: i32) -> Result<(), sqlx::Error> {
        // Scores cannot be negative
        if home_score < 0 {
            return Err(sqlx::Error::Protocol(
                format!("Home score cannot be negative: {}", home_score).into()
            ));
        }

        if away_score < 0 {
            return Err(sqlx::Error::Protocol(
                format!("Away score cannot be negative: {}", away_score).into()
            ));
        }

        // Reasonable upper limit (prevent obvious data entry errors)
        const MAX_REASONABLE_SCORE: i32 = 50;
        if home_score > MAX_REASONABLE_SCORE {
            return Err(sqlx::Error::Protocol(
                format!("Home score too high: {} (max {})", home_score, MAX_REASONABLE_SCORE).into()
            ));
        }

        if away_score > MAX_REASONABLE_SCORE {
            return Err(sqlx::Error::Protocol(
                format!("Away score too high: {} (max {})", away_score, MAX_REASONABLE_SCORE).into()
            ));
        }

        Ok(())
    }

    /// Validate team ID for operations
    pub fn validate_team_id(&self, team_id: Uuid) -> Result<(), sqlx::Error> {
        if team_id.is_nil() {
            return Err(sqlx::Error::Protocol("Team ID cannot be nil".into()));
        }
        Ok(())
    }

    /// Validate season ID
    pub fn validate_season_id(&self, season_id: Uuid) -> Result<(), sqlx::Error> {
        if season_id.is_nil() {
            return Err(sqlx::Error::Protocol("Season ID cannot be nil".into()));
        }
        Ok(())
    }

    /// Validate game ID
    pub fn validate_game_id(&self, game_id: Uuid) -> Result<(), sqlx::Error> {
        if game_id.is_nil() {
            return Err(sqlx::Error::Protocol("Game ID cannot be nil".into()));
        }
        Ok(())
    }

    /// Validate week number
    pub fn validate_week_number(&self, week_number: i32) -> Result<(), sqlx::Error> {
        if week_number < 1 {
            return Err(sqlx::Error::Protocol(
                format!("Week number must be positive: {}", week_number).into()
            ));
        }

        if week_number > 100 {
            return Err(sqlx::Error::Protocol(
                format!("Week number too high: {} (max 100)", week_number).into()
            ));
        }

        Ok(())
    }

    /// Validate pagination parameters
    pub fn validate_pagination(&self, limit: Option<i64>, offset: Option<i64>) -> Result<(i64, i64), sqlx::Error> {
        let limit = limit.unwrap_or(10);
        let offset = offset.unwrap_or(0);

        if limit < 1 {
            return Err(sqlx::Error::Protocol("Limit must be positive".into()));
        }

        if limit > 1000 {
            return Err(sqlx::Error::Protocol("Limit too high (max 1000)".into()));
        }

        if offset < 0 {
            return Err(sqlx::Error::Protocol("Offset cannot be negative".into()));
        }

        Ok((limit, offset))
    }

    /// Validate date range for queries
    pub fn validate_date_range(
        &self, 
        start_date: Option<DateTime<Utc>>, 
        end_date: Option<DateTime<Utc>>
    ) -> Result<(), sqlx::Error> {
        if let (Some(start), Some(end)) = (start_date, end_date) {
            if start >= end {
                return Err(sqlx::Error::Protocol("Start date must be before end date".into()));
            }

            // Check for unreasonable date ranges
            if end - start > Duration::days(3650) { // 10 years
                return Err(sqlx::Error::Protocol("Date range too large (max 10 years)".into()));
            }
        }

        Ok(())
    }

    /// Comprehensive input sanitization
    pub fn sanitize_string_input(&self, input: &str) -> String {
        input
            .trim()
            .chars()
            .filter(|&c| c != '\0') // Remove null bytes
            .collect::<String>()
            .trim()
            .to_string()
    }

    /// Validate and sanitize team name
    pub fn validate_and_sanitize_team_name(&self, name: &str) -> Result<String, sqlx::Error> {
        let sanitized = self.sanitize_string_input(name);
        
        if sanitized.is_empty() {
            return Err(sqlx::Error::Protocol("Team name cannot be empty".into()));
        }

        if sanitized.len() > 100 {
            return Err(sqlx::Error::Protocol("Team name too long (max 100 characters)".into()));
        }

        Ok(sanitized)
    }
}

impl Default for LeagueValidator {
    fn default() -> Self {
        Self::new()
    }
}