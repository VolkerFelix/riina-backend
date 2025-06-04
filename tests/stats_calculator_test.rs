use evolveme_backend::game::stats_calculator::{StatCalculator, StatChanges, GameStats};
use evolveme_backend::models::health_data::{HealthDataSyncRequest, SleepData};
use chrono::Utc;

#[test]
fn test_steps_calculation() {
    let health_data = HealthDataSyncRequest {
        device_id: "test".to_string(),
        timestamp: Utc::now(),
        steps: Some(5000),
        heart_rate: None,
        sleep: None,
        active_energy_burned: None,
        additional_metrics: None,
    };

    let changes = StatCalculator::calculate_stat_changes(&health_data);
    assert_eq!(changes.stamina_change, 5); // 5000 steps = 5 stamina
    assert_eq!(changes.experience_change, 50); // 5000 steps = 50 XP (5 XP per 1000 steps)
    assert!(changes.reasoning.len() > 0);
}

#[test]
fn test_sleep_calculation() {
    let sleep_data = SleepData {
        total_sleep_hours: 8.0,
        in_bed_time: None,
        out_bed_time: None,
        time_in_bed: None,
    };

    let health_data = HealthDataSyncRequest {
        device_id: "test".to_string(),
        timestamp: Utc::now(),
        steps: None,
        heart_rate: None,
        sleep: Some(sleep_data),
        active_energy_burned: None,
        additional_metrics: None,
    };

    let changes = StatCalculator::calculate_stat_changes(&health_data);
    assert_eq!(changes.mana_change, 4); // 8h sleep = 4 mana
    assert_eq!(changes.wisdom_change, 2); // Good sleep = 2 wisdom
}