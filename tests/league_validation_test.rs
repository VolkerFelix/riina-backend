use evolveme_backend::league::validation::LeagueValidator;
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;

#[test]
fn test_validate_season_name() {
    let validator = LeagueValidator::new();
    
    // Valid names
    assert!(validator.validate_season_name("Season 2024").is_ok());
    assert!(validator.validate_season_name("Spring League").is_ok());
    
    // Invalid names
    assert!(validator.validate_season_name("").is_err());
    assert!(validator.validate_season_name("   ").is_err());
    assert!(validator.validate_season_name(&"a".repeat(256)).is_err());
    assert!(validator.validate_season_name("!!!").is_err());
}

#[test]
fn test_validate_team_ids() {
    let validator = LeagueValidator::new();
    
    // Valid team IDs
    let valid_teams = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    assert!(validator.validate_team_ids(&valid_teams).is_ok());
    
    // Invalid - too few teams
    assert!(validator.validate_team_ids(&[Uuid::new_v4()]).is_err());
    
    // Invalid - duplicates
    let duplicate_team = Uuid::new_v4();
    assert!(validator.validate_team_ids(&[duplicate_team, duplicate_team]).is_err());
    
    // Invalid - too many teams
    let too_many_teams: Vec<Uuid> = (0..21).map(|_| Uuid::new_v4()).collect();
    assert!(validator.validate_team_ids(&too_many_teams).is_err());
}

#[test]
fn test_validate_game_scores() {
    let validator = LeagueValidator::new();
    
    // Valid scores
    assert!(validator.validate_game_scores(2, 1).is_ok());
    assert!(validator.validate_game_scores(0, 0).is_ok());
    assert!(validator.validate_game_scores(10, 8).is_ok());
    
    // Invalid scores
    assert!(validator.validate_game_scores(-1, 0).is_err());
    assert!(validator.validate_game_scores(0, -1).is_err());
    assert!(validator.validate_game_scores(100, 0).is_err());
}

#[test]
fn test_validate_start_date() {
    let validator = LeagueValidator::new();
    let now = Utc::now();
    
    // Valid dates
    assert!(validator.validate_start_date(now + Duration::hours(1)).is_ok());
    assert!(validator.validate_start_date(now + Duration::days(7)).is_ok());
    
    // Invalid dates
    assert!(validator.validate_start_date(now - Duration::days(1)).is_err());
    assert!(validator.validate_start_date(now + Duration::days(400)).is_err());
}

#[test]
fn test_sanitize_string_input() {
    let validator = LeagueValidator::new();
    
    assert_eq!(validator.sanitize_string_input("  test  "), "test");
    assert_eq!(validator.sanitize_string_input("test\0name"), "testname");
    assert_eq!(validator.sanitize_string_input("normal text"), "normal text");
} 