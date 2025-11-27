use chrono::Utc;

use riina_backend::workout::workout_analyzer::WorkoutAnalyzer;
use riina_backend::models::health::{HeartRateZones, HeartRateZoneName, Gender};
use riina_backend::models::workout_data::HeartRateData;

mod common;
use common::workout_data_helpers::{WorkoutData, WorkoutIntensity};

#[test]
fn test_millisecond_timestamps_produces_zone_time() {

    let workout_data = WorkoutData::new_with_hr_freq(WorkoutIntensity::Hard, Utc::now(), 110, Some(2));
    let heart_rate_data: Vec<HeartRateData> = workout_data.heart_rate.iter().map(|v| serde_json::from_value(v.clone()).expect("Failed to parse heart rate data")).collect();

    // Create heart rate zones for testing
    let resting_hr = 60;
    let age = 30; // Use a realistic age
    let zones = HeartRateZones::new(age, Gender::Male, resting_hr);

    // Analyze the workout
    let analyzer = WorkoutAnalyzer::new(
        &heart_rate_data,
        &zones
    );

    assert!(!analyzer.zone_durations.is_empty(), "WorkoutAnalyzer should have zone durations");

    println!("Total duration: {} minutes", analyzer.total_duration_min);
    println!("Zone durations:");
    for (zone, minutes) in &analyzer.zone_durations {
        println!("  {:?}: {:.2} minutes", zone, minutes);
    }

    let total_zone_time: f32 = analyzer.zone_durations.values().sum();
    println!("Total time in all zones: {:.2} minutes", total_zone_time);

    // After the fix, this should be approximately 110 minutes
    assert!(
        total_zone_time > 100.0,
        "Expected ~110 minutes in zones, but got {:.2} minutes", total_zone_time
    );

    // Calculate expected score
    let stamina: i32 = analyzer.zone_durations.iter().map(|(zone, minutes)| {
        let points_per_min = match zone {
            HeartRateZoneName::Zone1 => 2,
            HeartRateZoneName::Zone2 => 5,
            HeartRateZoneName::Zone3 => 4,
            HeartRateZoneName::Zone4 => 2,
            HeartRateZoneName::Zone5 => 1,
        };
        (minutes * points_per_min as f32) as i32
    }).sum();

    let strength: i32 = analyzer.zone_durations.iter().map(|(zone, minutes)| {
        let points_per_min = match zone {
            HeartRateZoneName::Zone1 => 0,
            HeartRateZoneName::Zone2 => 1,
            HeartRateZoneName::Zone3 => 3,
            HeartRateZoneName::Zone4 => 5,
            HeartRateZoneName::Zone5 => 8,
        };
        (minutes * points_per_min as f32) as i32
    }).sum();

    let total_score = stamina + strength;
    println!("Calculated score: stamina={}, strength={}, total={}", stamina, strength, total_score);

    assert!(total_score > 0, "Total score should be greater than 0, got {}", total_score);
}