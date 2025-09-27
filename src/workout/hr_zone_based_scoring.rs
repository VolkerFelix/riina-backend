use std::io::Error;

use crate::models::{
    workout_data::{WorkoutStats, HeartRateData, ZoneBreakdown},
    health::{HeartRateZones, HeartRateZoneName, UserHealthProfile},
};
use crate::game::stats_calculator::ScoringMethod;
use crate::workout::workout_analyzer::WorkoutAnalyzer;

// Stamina gains (cardiovascular endurance)
pub const ZONE_1_STAMINA_POINTS_PER_MIN: i32 = 2;  // Recovery still builds base
pub const ZONE_2_STAMINA_POINTS_PER_MIN: i32 = 5;  // Sweet spot for stamina
pub const ZONE_3_STAMINA_POINTS_PER_MIN: i32 = 4;  // Good stamina gains
pub const ZONE_4_STAMINA_POINTS_PER_MIN: i32 = 2;  // Some stamina benefit
pub const ZONE_5_STAMINA_POINTS_PER_MIN: i32 = 1;  // Minimal stamina gains
// Strength gains (power/anaerobic capacity)  
pub const ZONE_1_STRENGTH_POINTS_PER_MIN: i32 = 0;  // No strength from recovery
pub const ZONE_2_STRENGTH_POINTS_PER_MIN: i32 = 1;  // Minimal strength gains
pub const ZONE_3_STRENGTH_POINTS_PER_MIN: i32 = 3;  // Moderate strength gains
pub const ZONE_4_STRENGTH_POINTS_PER_MIN: i32 = 5;  // High strength gains
pub const ZONE_5_STRENGTH_POINTS_PER_MIN: i32 = 8;  // Maximum strength gains

pub struct HRZoneBasedScoring;

impl ScoringMethod for HRZoneBasedScoring {
    fn calculate_stats(&self, user_health_profile: UserHealthProfile, hr_data: Vec<HeartRateData>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<WorkoutStats, Error>> + Send + 'static>> {
        Box::pin(calculate_stats_hr_zone_based(user_health_profile, hr_data))
    }
}

async fn calculate_stats_hr_zone_based(user_health_profile: UserHealthProfile, hr_data: Vec<HeartRateData>) -> Result<WorkoutStats, Error> {
    // Use stored heart rate zones if available, otherwise calculate them
    let heart_rate_zones = if let Some(stored_zones) = user_health_profile.stored_heart_rate_zones {
        stored_zones
    } else {
        tracing::info!("⚠️ No stored zones found, calculating heart rate zones for age={}, gender={:?}, resting_hr={}",
            user_health_profile.age, user_health_profile.gender, user_health_profile.resting_heart_rate.unwrap_or(60));
        HeartRateZones::new(user_health_profile.age, user_health_profile.gender, user_health_profile.resting_heart_rate.unwrap_or(60))
    };
    
    // Check if heart rate data exists and is not empty
    if hr_data.is_empty() {
        tracing::warn!("⚠️ Heart rate data array is empty - returning zero stats");
        return Ok(WorkoutStats::new());
    }

    tracing::info!("📊 Processing {} heart rate data points", hr_data.len());
    let avg_hr: i32 = hr_data.iter().map(|hr| hr.heart_rate).sum::<i32>() / hr_data.len() as i32;
    tracing::info!("💗 Heart rate range: avg={:.1}, min={:.1}, max={:.1}",
        avg_hr,
        hr_data.iter().map(|hr| hr.heart_rate).fold(i32::MAX, i32::min),
        hr_data.iter().map(|hr| hr.heart_rate).fold(0, i32::max)
    );

    let workout_analysis = WorkoutAnalyzer::new(&hr_data, &heart_rate_zones);
    tracing::info!("✅ WorkoutAnalyzer created successfully");
    tracing::info!("🎯 Heart rate zones: {:?}", heart_rate_zones.zones);
    for (zone, minutes) in &workout_analysis.zone_durations {
        tracing::info!("📈 Zone {:?}: {:.1} minutes", zone, minutes);
    }
    let workout_stats = calc_points_and_breakdown_from_workout_analysis(&workout_analysis, &heart_rate_zones);

    tracing::info!("🎯 Final stat changes: stamina +{}, strength +{}",
        workout_stats.changes.stamina_change, workout_stats.changes.strength_change);
    Ok(workout_stats)
}

fn calc_points_and_breakdown_from_workout_analysis(workout_analysis: &WorkoutAnalyzer, heart_rate_zones: &HeartRateZones) -> WorkoutStats {
    let mut workout_stats = WorkoutStats::new();

    let mut total_stamina = 0.0;
    let mut total_strength = 0.0;
    let mut zone_breakdown = Vec::new();

    for (zone, duration_minutes) in &workout_analysis.zone_durations {
        let (stamina_per_min, strength_per_min) = match zone {
            HeartRateZoneName::Zone1 => (ZONE_1_STAMINA_POINTS_PER_MIN, ZONE_1_STRENGTH_POINTS_PER_MIN),
            HeartRateZoneName::Zone2 => (ZONE_2_STAMINA_POINTS_PER_MIN, ZONE_2_STRENGTH_POINTS_PER_MIN),
            HeartRateZoneName::Zone3 => (ZONE_3_STAMINA_POINTS_PER_MIN, ZONE_3_STRENGTH_POINTS_PER_MIN),
            HeartRateZoneName::Zone4 => (ZONE_4_STAMINA_POINTS_PER_MIN, ZONE_4_STRENGTH_POINTS_PER_MIN),
            HeartRateZoneName::Zone5 => (ZONE_5_STAMINA_POINTS_PER_MIN, ZONE_5_STRENGTH_POINTS_PER_MIN),
        };
        
        let zone_stamina = (duration_minutes * stamina_per_min as f32) as i32;
        let zone_strength = (duration_minutes * strength_per_min as f32) as i32;
        
        total_stamina += zone_stamina as f32;
        total_strength += zone_strength as f32;

        // Get heart rate limits for this zone from the user's zones
        let (hr_min, hr_max) = if let Some(zone_range) = heart_rate_zones.zones.get(zone) {
            (Some(zone_range.low), Some(zone_range.high))
        } else {
            (None, None)
        };

        // Add zone breakdown for this zone
        zone_breakdown.push(ZoneBreakdown {
            zone: format!("{}", zone),
            minutes: *duration_minutes,
            stamina_gained: zone_stamina as f32,
            strength_gained: zone_strength as f32,
            hr_min,
            hr_max,
        });
    }

    workout_stats.changes.stamina_change = total_stamina as f32;
    workout_stats.changes.strength_change = total_strength as f32;
    workout_stats.zone_breakdown = Some(zone_breakdown);
    workout_stats
}