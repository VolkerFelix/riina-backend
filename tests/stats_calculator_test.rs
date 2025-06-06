use evolveme_backend::game::stats_calculator::StatCalculator;
use evolveme_backend::models::health_data::HealthDataSyncRequest;
use chrono::Utc;

#[test]
fn test_zone_1_active_recovery() {
    // Create a vector of a sinus wave with a period of 10 seconds
    let heart_rate_data = vec![HeartRateData {
        timestamp: Utc::now(),
        heart_rate: 55.0,
    }];
    for i in 0..10 {
        heart_rate_data.push(HeartRateData {
            timestamp: Utc::now() + Duration::seconds(i),
            heart_rate: 55.0 + 2.0 * (i as f32).sin(),
        });
    }
    let health_data = HealthDataSyncRequest {
        device_id: "test".to_string(),
        timestamp: Utc::now(),
        heart_rate: Some(heart_rate_data),
        active_energy_burned: Some(150.0),
    };

    let changes = StatCalculator::calculate_stat_changes(&health_data);
    assert_eq!(changes.stamina_change, 1); // Minimal gains in Zone 1
    assert_eq!(changes.strength_change, 0);
    assert!(changes.reasoning.len() > 0);
    assert!(changes.reasoning[0].contains("Active Recovery"));
}

#[test]
fn test_zone_2_aerobic_base() {
    // Create a vector of a sinus wave with a period of 10 seconds
    let heart_rate_data = vec![HeartRateData {
        timestamp: Utc::now(),
        heart_rate: 67.0,
    }];
    for i in 0..10 {
        heart_rate_data.push(HeartRateData {
            timestamp: Utc::now() + Duration::seconds(i),
            heart_rate: 67.0 + 2.0 * (i as f32).sin(),
        });
    }
    let health_data = HealthDataSyncRequest {
        device_id: "test".to_string(),
        timestamp: Utc::now(),
        heart_rate: Some(heart_rate_data),
        active_energy_burned: Some(225.0),
    };

    let changes = StatCalculator::calculate_stat_changes(&health_data);
    assert_eq!(changes.stamina_change, 3); // Primary stamina gains
    assert_eq!(changes.strength_change, 0);
    assert!(changes.reasoning[0].contains("Aerobic Base"));
}

#[test]
fn test_zone_4_lactate_threshold() {
    let heart_rate_data = vec![HeartRateData {
        timestamp: Utc::now(),
        heart_rate: 85.0,
    }];
    for i in 0..10 {
        heart_rate_data.push(HeartRateData {
            timestamp: Utc::now() + Duration::seconds(i),
            heart_rate: 85.0 + 2.0 * (i as f32).sin(),
        });
    }
    let health_data = HealthDataSyncRequest {
        device_id: "test".to_string(),
        timestamp: Utc::now(),
        heart_rate: Some(heart_rate_data),
        active_energy_burned: Some(300.0),
    };

    let changes = StatCalculator::calculate_stat_changes(&health_data);
    assert_eq!(changes.stamina_change, 3); // Balanced gains
    assert_eq!(changes.strength_change, 2);
    assert!(changes.reasoning[0].contains("Lactate Threshold"));
}

#[test]
fn test_zone_5_neuromuscular_power() {
    let heart_rate_data = vec![HeartRateData {
        timestamp: Utc::now(),
        heart_rate: 100.0,
    }];
    for i in 0..10 {
        heart_rate_data.push(HeartRateData {
            timestamp: Utc::now() + Duration::seconds(i),
            heart_rate: 100.0 + 2.0 * (i as f32).sin(),
        });
    }
    let health_data = HealthDataSyncRequest {
        device_id: "test".to_string(),
        timestamp: Utc::now(),
        heart_rate: Some(heart_rate_data),
        active_energy_burned: Some(400.0),
    };

    let changes = StatCalculator::calculate_stat_changes(&health_data);
    assert_eq!(changes.stamina_change, 2); // Less stamina
    assert_eq!(changes.strength_change, 4); // Primary strength gains
    assert_eq!(changes.experience_change, 80); // 60 base + 20 Zone 5 bonus
    assert!(changes.reasoning[0].contains("Neuromuscular Power"));
}

#[test]
fn test_no_heart_rate_no_gains() {
    let health_data = HealthDataSyncRequest {
        device_id: "test".to_string(),
        timestamp: Utc::now(),
        heart_rate: None,
        active_energy_burned: Some(200.0),
    };

    let changes = StatCalculator::calculate_stat_changes(&health_data);
    assert_eq!(changes.stamina_change, 0);
    assert_eq!(changes.strength_change, 0);
    assert_eq!(changes.experience_change, 0);
    assert_eq!(changes.reasoning.len(), 0);
}