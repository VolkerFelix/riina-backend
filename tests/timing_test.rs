use chrono::{TimeZone, Utc, Datelike, Timelike, Weekday};
use evolveme_backend::league::timing::{TimingService, UrgencyLevel};

#[test]
fn test_next_game_time_is_saturday() {
    let service = TimingService::new();
    let next_game = service.get_next_game_time();
    assert_eq!(next_game.weekday(), Weekday::Sat);
    assert_eq!(next_game.hour(), 22);
    assert_eq!(next_game.minute(), 0);
}

#[test]
fn test_countdown_formatting() {
    let service = TimingService::new();
    assert_eq!(service.format_countdown(0), "Game time!");
    assert_eq!(service.format_countdown(30), "30s");
    assert_eq!(service.format_countdown(90), "1m 30s");
    assert_eq!(service.format_countdown(3661), "1h 1m 1s");
    assert_eq!(service.format_countdown(90061), "1d 1h 1m 1s");
}

#[test]
fn test_urgency_levels() {
    let service = TimingService::new();
    assert_eq!(service.get_urgency_level(0), UrgencyLevel::GameTime);
    assert_eq!(service.get_urgency_level(1800), UrgencyLevel::Critical);
    assert_eq!(service.get_urgency_level(10800), UrgencyLevel::High);
    assert_eq!(service.get_urgency_level(43200), UrgencyLevel::Medium);
    assert_eq!(service.get_urgency_level(172800), UrgencyLevel::Low);
} 