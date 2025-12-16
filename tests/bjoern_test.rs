use chrono::{NaiveDate, NaiveTime, Utc, TimeZone};
use riina_backend::models::workout_data::{HeartRateData, WorkoutType};
use riina_backend::models::health::{Gender, TrainingZones, UserHealthProfile};
use riina_backend::workout::universal_hr_based_scoring::UniversalHRBasedScoring;
use riina_backend::game::stats_calculator::ScoringMethod;
use riina_backend::utils::heart_rate_filters::filter_heart_rate_data;

// Heart rate data from the bug report including duplicated timestamps at "13:59:48"
static HR_SAMPLES: [(&str, i32); 212] = [
    ("13:51:59", 74), ("13:52:00", 78), ("13:52:04", 92), ("13:52:08", 106),
    ("13:52:09", 109), ("13:52:14", 120), ("13:52:22", 128), ("13:52:32", 131),
    ("13:52:40", 133), ("13:52:55", 139), ("13:53:03", 142), ("13:53:05", 143),
    ("13:53:12", 144), ("13:53:28", 146), ("13:53:39", 145), ("13:53:50", 145),
    ("13:54:01", 145), ("13:54:13", 144), ("13:54:21", 144), ("13:54:31", 145),
    ("13:54:38", 144), ("13:54:53", 147), ("13:55:03", 147), ("13:55:13", 151),
    ("13:55:23", 148), ("13:55:32", 146), ("13:55:40", 146), ("13:55:52", 146),
    ("13:55:59", 146), ("13:56:05", 143), ("13:56:14", 144), ("13:56:20", 145),
    ("13:56:25", 146), ("13:56:32", 146), ("13:56:41", 148), ("13:56:53", 150),
    ("13:57:05", 151), ("13:57:14", 150), ("13:57:24", 151), ("13:57:38", 149),
    ("13:57:50", 148), ("13:57:59", 150), ("13:58:11", 151), ("13:58:23", 154),
    ("13:58:37", 159), ("13:58:50", 156), ("13:59:04", 154), ("13:59:21", 152),
    ("13:59:35", 151), ("13:59:48", 153), ("13:59:48", 150), ("13:59:53", 149),
    ("14:00:01", 151), ("14:00:14", 147), ("14:00:25", 147), ("14:00:40", 148),
    ("14:00:54", 150), ("14:01:03", 150), ("14:01:11", 151), ("14:01:24", 153),
    ("14:01:37", 154), ("14:01:51", 155), ("14:02:10", 154), ("14:02:20", 157),
    ("14:02:36", 155), ("14:02:41", 156), ("14:02:45", 139), ("14:02:53", 140),
    ("14:02:59", 140), ("14:03:05", 139), ("14:03:11", 138), ("14:03:19", 141),
    ("14:03:30", 141), ("14:03:39", 140), ("14:03:47", 141), ("14:04:00", 143),
    ("14:04:17", 149), ("14:04:30", 148), ("14:04:44", 149), ("14:04:55", 151),
    ("14:05:09", 151), ("14:05:18", 150), ("14:05:20", 151), ("14:05:31", 151),
    ("14:05:42", 152), ("14:05:55", 151), ("14:06:08", 152), ("14:06:20", 152),
    ("14:06:33", 156), ("14:06:50", 156), ("14:07:03", 156), ("14:07:07", 154),
    ("14:07:29", 156), ("14:07:49", 159), ("14:07:55", 159), ("14:08:01", 160),
    ("14:08:19", 159), ("14:08:31", 153), ("14:08:48", 151), ("14:09:06", 151),
    ("14:09:26", 155), ("14:09:41", 151), ("14:09:51", 152), ("14:10:07", 152),
    ("14:10:19", 155), ("14:10:31", 158), ("14:10:36", 161), ("14:10:38", 161),
    ("14:10:43", 162), ("14:10:56", 158), ("14:11:10", 159), ("14:11:20", 158),
    ("14:11:32", 153), ("14:11:46", 157), ("14:12:00", 154), ("14:12:12", 159),
    ("14:12:24", 158), ("14:12:38", 156), ("14:12:53", 159), ("14:13:04", 138),
    ("14:13:08", 131), ("14:13:12", 155), ("14:13:25", 158), ("14:13:39", 154),
    ("14:13:51", 153), ("14:14:02", 150), ("14:14:18", 153), ("14:14:28", 152),
    ("14:14:38", 156), ("14:14:53", 161), ("14:15:05", 159), ("14:15:20", 155),
    ("14:15:40", 153), ("14:15:54", 150), ("14:16:02", 150), ("14:16:19", 151),
    ("14:16:30", 152), ("14:16:39", 156), ("14:16:50", 160), ("14:17:00", 162),
    ("14:17:15", 158), ("14:17:41", 160), ("14:17:59", 156), ("14:18:18", 151),
    ("14:18:39", 150), ("14:18:57", 148), ("14:19:16", 147), ("14:19:33", 151),
    ("14:19:36", 150), ("14:19:53", 150), ("14:20:05", 147), ("14:20:22", 148),
    ("14:20:35", 153), ("14:20:45", 154), ("14:20:58", 156), ("14:21:12", 156),
    ("14:21:30", 153), ("14:21:44", 158), ("14:21:59", 155), ("14:22:13", 153),
    ("14:22:27", 153), ("14:22:35", 155), ("14:22:49", 160), ("14:23:02", 158),
    ("14:23:17", 158), ("14:23:28", 164), ("14:23:38", 164), ("14:23:53", 165),
    ("14:24:07", 165), ("14:24:13", 144), ("14:24:21", 141), ("14:24:32", 149),
    ("14:24:49", 155), ("14:25:03", 156), ("14:25:15", 160), ("14:25:27", 164),
    ("14:25:40", 166), ("14:25:50", 168), ("14:26:01", 169), ("14:26:14", 168),
    ("14:26:27", 168), ("14:26:33", 167), ("14:26:50", 167), ("14:27:07", 170),
    ("14:27:19", 163), ("14:27:28", 163), ("14:27:41", 163), ("14:27:53", 169),
    ("14:28:06", 167), ("14:28:19", 167), ("14:28:30", 167), ("14:28:41", 169),
    ("14:28:48", 174), ("14:28:55", 182), ("14:29:04", 188), ("14:29:11", 189),
    ("14:29:15", 191), ("14:29:18", 191), ("14:29:25", 191), ("14:29:30", 189),
    ("14:29:34", 185), ("14:29:44", 178), ("14:29:59", 174), ("14:30:12", 171),
    ("14:30:28", 168), ("14:30:40", 168), ("14:30:44", 168), ("14:30:56", 167),
    ("14:31:07", 161), ("14:31:08", 161), ("14:31:19", 157), ("14:31:22", 156),
];

#[tokio::test]
async fn test_workout_zone_duration_bug() {

    let date = NaiveDate::from_ymd_opt(2025, 11, 17).unwrap();
    let mut hr_data: Vec<HeartRateData> = HR_SAMPLES
        .into_iter()
        .map(|(time_str, hr)| {
            let time = NaiveTime::parse_from_str(time_str, "%H:%M:%S").unwrap();
            let naive_dt = date.and_time(time);
            HeartRateData { timestamp: Utc.from_utc_datetime(&naive_dt), heart_rate: hr }
        })
        .collect();

    // Apply the same filter that the backend applies during workout upload
    let workout_start = hr_data.first().unwrap().timestamp;
    let workout_end = hr_data.last().unwrap().timestamp;
    let removed_samples = filter_heart_rate_data(&mut hr_data, &workout_start, &workout_end);
    println!("\n⚠️ FILTER APPLIED: Removed {} HR samples due to duplicate/non-increasing timestamps", removed_samples);

    // Calculate expected workout duration
    let start_time = hr_data.first().unwrap().timestamp;
    let end_time = hr_data.last().unwrap().timestamp;
    let expected_duration_min = (end_time - start_time).num_seconds() as f32 / 60.0;

    // Zone parameters (typical values)
    let hr_rest = 50;
    let hr_max = 189;

    // Create user health profile with the test parameters
    let user_health_profile = UserHealthProfile {
        age: 30,
        gender: Gender::Male,
        resting_heart_rate: hr_rest,
        max_heart_rate: hr_max
    };

    println!("=== WORKOUT INFO ===");
    println!("Expected workout duration: {:.2} minutes", expected_duration_min);
    println!("Total HR samples: {}", hr_data.len());

    // Use the actual backend logic to calculate workout stats
    let scoring_method = UniversalHRBasedScoring;
    let workout_type = WorkoutType::Cardio;
    let workout_stats = scoring_method.calculate_stats(user_health_profile, hr_data.clone(), workout_type).await
        .expect("Failed to calculate workout stats");

    let zone_breakdown = workout_stats.zone_breakdown.as_ref()
        .expect("Zone breakdown should be present");

    let total_zone_duration: f32 = zone_breakdown.iter()
        .map(|zone| zone.minutes)
        .sum();

    println!("\n=== ZONE BREAKDOWN (from backend logic) ===");
    for zone in zone_breakdown {
        let pct = if total_zone_duration > 0.0 { (zone.minutes / total_zone_duration) * 100.0 } else { 0.0 };
        println!("{}: {:.1} min ({:.1}%) - stamina: {:.2} pts",
            zone.zone, zone.minutes, pct, zone.stamina_gained);
    }

    println!("\n=== SCORE SUMMARY ===");
    println!("Total stamina points: {:.2}", workout_stats.changes.stamina_change);

    println!("\n=== DURATION COMPARISON ===");
    println!("Total from zone breakdown: {:.2} minutes", total_zone_duration);
    println!("Expected workout duration: {:.2} minutes", expected_duration_min);
    println!("Difference: {:.2} minutes ({:.1}%)",
        expected_duration_min - total_zone_duration,
        ((expected_duration_min - total_zone_duration) / expected_duration_min) * 100.0
    );

    // Show HR distribution
    println!("\n=== HEART RATE DISTRIBUTION ===");
    let hr_values: Vec<i32> = hr_data.iter().map(|d| d.heart_rate).collect();
    let min_hr = hr_values.iter().min().unwrap();
    let max_hr = hr_values.iter().max().unwrap();
    let avg_hr = hr_values.iter().sum::<i32>() as f32 / hr_values.len() as f32;
    println!("Min HR: {} bpm", min_hr);
    println!("Max HR: {} bpm", max_hr);
    println!("Avg HR: {:.1} bpm", avg_hr);

    // Confirm zone durations approximately match workout duration
    assert!(
        (total_zone_duration - expected_duration_min).abs() < 0.1,
        "Zone durations ({:.2} min) should approximately equal workout duration ({:.2} min)",
        total_zone_duration, expected_duration_min
    );

    println!("\n✅ Test passed: Zone breakdown correctly accounts for workout duration");
}

#[tokio::test]
async fn test_workout_time_range_filtering() {
    let date = NaiveDate::from_ymd_opt(2025, 11, 17).unwrap();
    let mut hr_data: Vec<HeartRateData> = HR_SAMPLES
        .into_iter()
        .map(|(time_str, hr)| {
            let time = NaiveTime::parse_from_str(time_str, "%H:%M:%S").unwrap();
            let naive_dt = date.and_time(time);
            HeartRateData { timestamp: Utc.from_utc_datetime(&naive_dt), heart_rate: hr }
        })
        .collect();

    let workout_start = hr_data.first().unwrap().timestamp;
    // Iterate 5 elements back from the last element
    let workout_end = hr_data.iter().rev().nth(5).unwrap().timestamp;
    let removed_samples = filter_heart_rate_data(&mut hr_data, &workout_start, &workout_end);
    println!("\n⚠️ FILTER APPLIED: Removed {} HR samples due to duplicate/non-increasing timestamps", removed_samples);
    // Should remove 6 samples (last 5 + the duplicate)
    assert!(hr_data.len() == 206, "Expected 206 HR samples, got {}", hr_data.len());
    assert!(removed_samples == 6, "Expected 6 HR samples to be removed, got {}", removed_samples);
}