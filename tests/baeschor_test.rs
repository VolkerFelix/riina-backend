use chrono::Utc;

use riina_backend::workout::workout_analyzer::WorkoutAnalyzer;
use riina_backend::models::workout_data::{HeartRateData, HeartRateZones, ZoneName};
mod common;
use common::workout_data_helpers::{WorkoutData, WorkoutType};

#[test]
fn test_millisecond_timestamps_produces_zone_time() {

    let workout_data = WorkoutData::new_with_hr_freq(WorkoutType::Hard, Utc::now(), 110, Some(2));
    let heart_rate_data = workout_data.heart_rate.iter().map(|v| serde_json::from_value(v.clone()).expect("Failed to parse heart rate data")).collect();

    // Create heart rate zones for testing
    let resting_hr = 60;
    let max_hr = 200;
    let hrr = max_hr - resting_hr;
    let zones = HeartRateZones::new(hrr, resting_hr, max_hr);

    // Analyze the workout
    let analyzer = WorkoutAnalyzer::new(
        &heart_rate_data,
        &zones
    );

    assert!(analyzer.is_some(), "WorkoutAnalyzer should be created");
    let analyzer = analyzer.unwrap();

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
            ZoneName::Zone1 => 2,
            ZoneName::Zone2 => 5,
            ZoneName::Zone3 => 4,
            ZoneName::Zone4 => 2,
            ZoneName::Zone5 => 1,
        };
        (minutes * points_per_min as f32) as i32
    }).sum();

    let strength: i32 = analyzer.zone_durations.iter().map(|(zone, minutes)| {
        let points_per_min = match zone {
            ZoneName::Zone1 => 0,
            ZoneName::Zone2 => 1,
            ZoneName::Zone3 => 3,
            ZoneName::Zone4 => 5,
            ZoneName::Zone5 => 8,
        };
        (minutes * points_per_min as f32) as i32
    }).sum();

    let total_score = stamina + strength;
    println!("Calculated score: stamina={}, strength={}, total={}", stamina, strength, total_score);

    assert!(total_score > 0, "Total score should be greater than 0, got {}", total_score);
}