use chrono::{DateTime, Utc, Datelike, Duration, Weekday};
use serde_json::json;

/// Service responsible for all time calculations and countdown logic
pub struct CountdownService;

impl CountdownService {
    pub fn new() -> Self {
        Self
    }

    /// Get the next Saturday at 22:00 UTC (10pm) from now
    /// This is the core algorithm for determining when games are played
    pub fn get_next_game_time(&self) -> DateTime<Utc> {
        let now = Utc::now();
        
        // Calculate days until next Saturday (0 = Monday, 5 = Saturday, 6 = Sunday)
        let current_weekday = now.weekday().num_days_from_monday();
        let days_until_saturday = if current_weekday <= 5 {
            5 - current_weekday // Days until this Saturday
        } else {
            6 // Days until next Saturday (if it's Sunday)
        };
        
        // Get next Saturday date
        let saturday_date = now.date_naive() + Duration::days(days_until_saturday as i64);
        let saturday_10pm = saturday_date.and_hms_opt(22, 0, 0)
            .expect("Invalid time: Saturday 22:00 should always be valid");
        let saturday_10pm_utc = DateTime::from_naive_utc_and_offset(saturday_10pm, Utc);
        
        // If it's already past this Saturday 10pm, get next Saturday
        if now >= saturday_10pm_utc {
            saturday_10pm_utc + Duration::weeks(1)
        } else {
            saturday_10pm_utc
        }
    }

    /// Calculate seconds until the next game time
    pub fn seconds_until_next_game(&self) -> i64 {
        let next_game_time = self.get_next_game_time();
        let now = Utc::now();
        (next_game_time - now).num_seconds().max(0)
    }

    /// Calculate game time for a specific week number
    /// Each week's game is on Saturday at 10pm UTC
    pub fn calculate_game_time_for_week(&self, start_date: DateTime<Utc>, week_number: i32) -> DateTime<Utc> {
        // Validate inputs
        if week_number < 1 {
            tracing::warn!("Invalid week number: {}, defaulting to week 1", week_number);
            return self.get_next_game_time();
        }

        // Calculate target date
        let weeks_to_add = (week_number - 1) as i64;
        let target_date = start_date + Duration::weeks(weeks_to_add);
        
        // Ensure it's on a Saturday
        let target_weekday = target_date.weekday().num_days_from_monday(); // 0=Mon, 5=Sat
        let days_to_saturday = if target_weekday <= 5 {
            5 - target_weekday
        } else {
            6 // If Sunday, go to next Saturday
        };
        
        let saturday_date = target_date.date_naive() + Duration::days(days_to_saturday as i64);
        let saturday_10pm = saturday_date.and_hms_opt(22, 0, 0)
            .unwrap_or_else(|| {
                tracing::error!("Failed to create Saturday 10pm time, using fallback");
                saturday_date.and_hms_opt(22, 0, 0).unwrap()
            });
        
        let result = DateTime::from_naive_utc_and_offset(saturday_10pm, Utc);
        
        tracing::debug!(
            "Calculated game time for week {}: {} ({})",
            week_number,
            result,
            result.format("%A, %B %d at %H:%M UTC")
        );
        
        result
    }

    /// Check if we're currently within game time (Saturday evening)
    pub fn is_game_time(&self) -> bool {
        let now = Utc::now();
        let next_game_time = self.get_next_game_time();
        
        // Check if we're within 2 hours of game time (before or after)
        (now - next_game_time).abs() <= Duration::hours(2)
    }

    /// Check if it's currently Saturday night (regardless of specific game time)
    pub fn is_saturday_night(&self) -> bool {
        let now = Utc::now();
        now.weekday() == Weekday::Sat && now.hour() >= 20 && now.hour() <= 23
    }

    /// Format countdown time in human-readable format
    pub fn format_countdown(&self, seconds: i64) -> String {
        if seconds <= 0 {
            return "Game time!".to_string();
        }

        let days = seconds / 86400;
        let hours = (seconds % 86400) / 3600;
        let minutes = (seconds % 3600) / 60;
        let secs = seconds % 60;

        if days > 0 {
            format!("{}d {}h {}m {}s", days, hours, minutes, secs)
        } else if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, secs)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, secs)
        } else {
            format!("{}s", secs)
        }
    }

    /// Get detailed countdown breakdown
    pub fn get_countdown_breakdown(&self) -> CountdownBreakdown {
        let seconds = self.seconds_until_next_game();
        let next_game_time = self.get_next_game_time();
        
        CountdownBreakdown {
            total_seconds: seconds,
            days: seconds / 86400,
            hours: (seconds % 86400) / 3600,
            minutes: (seconds % 3600) / 60,
            seconds: seconds % 60,
            formatted: self.format_countdown(seconds),
            next_game_time,
            is_game_time: self.is_game_time(),
            is_saturday_night: self.is_saturday_night(),
        }
    }

    /// Calculate time between two dates in a human-readable format
    pub fn format_duration_between(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> String {
        let duration = end - start;
        let total_seconds = duration.num_seconds().abs();
        
        let days = total_seconds / 86400;
        let hours = (total_seconds % 86400) / 3600;
        let minutes = (total_seconds % 3600) / 60;

        if days > 0 {
            format!("{} days, {} hours", days, hours)
        } else if hours > 0 {
            format!("{} hours, {} minutes", hours, minutes)
        } else {
            format!("{} minutes", minutes)
        }
    }

    /// Get urgency level based on time remaining
    pub fn get_urgency_level(&self, seconds_remaining: i64) -> UrgencyLevel {
        match seconds_remaining {
            0 => UrgencyLevel::GameTime,
            1..=3600 => UrgencyLevel::Critical,      // Less than 1 hour
            3601..=21600 => UrgencyLevel::High,      // Less than 6 hours
            21601..=86400 => UrgencyLevel::Medium,   // Less than 1 day
            _ => UrgencyLevel::Low,                  // More than 1 day
        }
    }

    /// Get next few game times for planning purposes
    pub fn get_upcoming_game_times(&self, count: usize) -> Vec<DateTime<Utc>> {
        let start_time = self.get_next_game_time();
        (0..count)
            .map(|i| start_time + Duration::weeks(i as i64))
            .collect()
    }

    /// Validate that a given time is a valid game time (Saturday 10pm)
    pub fn is_valid_game_time(&self, time: DateTime<Utc>) -> bool {
        time.weekday() == Weekday::Sat && time.hour() == 22 && time.minute() == 0 && time.second() == 0
    }
}

/// Detailed breakdown of countdown information
#[derive(Debug, Clone)]
pub struct CountdownBreakdown {
    pub total_seconds: i64,
    pub days: i64,
    pub hours: i64,
    pub minutes: i64,
    pub seconds: i64,
    pub formatted: String,
    pub next_game_time: DateTime<Utc>,
    pub is_game_time: bool,
    pub is_saturday_night: bool,
}

/// Urgency level for countdown display
#[derive(Debug, Clone, PartialEq)]
pub enum UrgencyLevel {
    GameTime,  // Game is happening now
    Critical,  // Less than 1 hour
    High,      // Less than 6 hours
    Medium,    // Less than 1 day
    Low,       // More than 1 day
}

impl UrgencyLevel {
    /// Get color hex code for UI display
    pub fn color(&self) -> &'static str {
        match self {
            UrgencyLevel::GameTime => "#FF0000", // Red
            UrgencyLevel::Critical => "#FF4444", // Bright red
            UrgencyLevel::High => "#FF8800",     // Orange
            UrgencyLevel::Medium => "#FFAA00",   // Yellow-orange
            UrgencyLevel::Low => "#4F46E5",      // Blue (primary)
        }
    }

    /// Get urgency message for display
    pub fn message(&self) -> &'static str {
        match self {
            UrgencyLevel::GameTime => "🔴 LIVE NOW!",
            UrgencyLevel::Critical => "🔥 Game starts very soon!",
            UrgencyLevel::High => "⏰ Game starts soon!",
            UrgencyLevel::Medium => "📅 Game tomorrow!",
            UrgencyLevel::Low => "🎮 Next game:",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_next_game_time_is_saturday() {
        let service = CountdownService::new();
        let next_game = service.get_next_game_time();
        assert_eq!(next_game.weekday(), Weekday::Sat);
        assert_eq!(next_game.hour(), 22);
        assert_eq!(next_game.minute(), 0);
    }

    #[test]
    fn test_countdown_formatting() {
        let service = CountdownService::new();
        assert_eq!(service.format_countdown(0), "Game time!");
        assert_eq!(service.format_countdown(30), "30s");
        assert_eq!(service.format_countdown(90), "1m 30s");
        assert_eq!(service.format_countdown(3661), "1h 1m 1s");
        assert_eq!(service.format_countdown(90061), "1d 1h 1m 1s");
    }

    #[test]
    fn test_valid_game_time() {
        let service = CountdownService::new();
        let valid_time = Utc.ymd(2024, 1, 6).and_hms(22, 0, 0); // Saturday 10pm
        let invalid_time = Utc.ymd(2024, 1, 6).and_hms(21, 0, 0); // Saturday 9pm
        
        assert!(service.is_valid_game_time(valid_time));
        assert!(!service.is_valid_game_time(invalid_time));
    }

    #[test]
    fn test_urgency_levels() {
        let service = CountdownService::new();
        assert_eq!(service.get_urgency_level(0), UrgencyLevel::GameTime);
        assert_eq!(service.get_urgency_level(1800), UrgencyLevel::Critical);
        assert_eq!(service.get_urgency_level(10800), UrgencyLevel::High);
        assert_eq!(service.get_urgency_level(43200), UrgencyLevel::Medium);
        assert_eq!(service.get_urgency_level(172800), UrgencyLevel::Low);
    }
}