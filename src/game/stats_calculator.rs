use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::models::game::*;
use crate::models::workout_data::{WorkoutDataUploadRequest, HeartRateZones, ZoneName, ZoneBreakdown, WorkoutStats};
use crate::game::helper::{get_user_profile, calc_max_heart_rate};
use crate::workout::workout_analyzer::WorkoutAnalyzer;

pub struct WorkoutStatsCalculator;

impl WorkoutStatsCalculator {
    /// Calculate base stats from HRR zones based on heart rate
    pub async fn calculate_stat_changes(pool: &Pool<Postgres>, user_id: Uuid, workout_data: &WorkoutDataUploadRequest) -> Result<WorkoutStats, Box<dyn std::error::Error>> {
        let user_profile = get_user_profile(pool, user_id).await.unwrap();
        
        // Use stored heart rate zones if available, otherwise calculate them
        let heart_rate_zones = if let Some(stored_zones) = user_profile.stored_heart_rate_zones {
            tracing::info!("📊 Using stored heart rate zones from database");
            stored_zones
        } else {
            tracing::info!("⚠️ No stored zones found, calculating heart rate zones");
            let max_heart_rate = user_profile.max_heart_rate.unwrap_or_else(|| 
                calc_max_heart_rate(user_profile.age, user_profile.gender)
            );
            let resting_heart_rate = user_profile.resting_heart_rate.unwrap_or(60);
            let hrr = max_heart_rate - resting_heart_rate;
            
            tracing::info!("💓 Heart rate calculation: max_hr={}, resting_hr={}, hrr={}", 
                max_heart_rate, resting_heart_rate, hrr);
            
            HeartRateZones::new(hrr, resting_heart_rate, max_heart_rate)
        };
        
        // Check if heart rate data exists and is not empty
        if let Some(ref heart_rate_data) = workout_data.heart_rate {
            if heart_rate_data.is_empty() {
                tracing::warn!("⚠️ Heart rate data array is empty - returning zero stats");
                let mut workout_stats = WorkoutStats::new();
                workout_stats.changes.stamina_change = 0;
                workout_stats.changes.strength_change = 0;
                return Ok(workout_stats);
            }

            tracing::info!("📊 Processing {} heart rate data points", heart_rate_data.len());
            let avg_hr: i32 = heart_rate_data.iter().map(|hr| hr.heart_rate).sum::<i32>() / heart_rate_data.len() as i32;
            tracing::info!("💗 Heart rate range: avg={:.1}, min={:.1}, max={:.1}",
                avg_hr,
                heart_rate_data.iter().map(|hr| hr.heart_rate).fold(i32::MAX, i32::min),
                heart_rate_data.iter().map(|hr| hr.heart_rate).fold(0, i32::max)
            );

            if let Some(workout_analysis) = WorkoutAnalyzer::new(heart_rate_data, &heart_rate_zones) {
                tracing::info!("✅ WorkoutAnalyzer created successfully");
                for (zone, minutes) in &workout_analysis.zone_durations {
                    tracing::info!("📈 Zone {:?}: {:.1} minutes", zone, minutes);
                }
                let workout_stats = Self::calc_points_and_breakdown_from_workout_analysis(&workout_analysis, &heart_rate_zones);

                tracing::info!("🎯 Final stat changes: stamina +{}, strength +{}",
                    workout_stats.changes.stamina_change, workout_stats.changes.strength_change);
                return Ok(workout_stats);
            } else {
                tracing::error!("❌ WorkoutAnalyzer::new() returned None - no stats calculated");
            }
        } else {
            tracing::warn!("⚠️ No heart rate data provided - returning zero stats");
        }

        // Return zero stats if no heart rate data or workout analysis failed
        let mut workout_stats = WorkoutStats::new();
        workout_stats.changes.stamina_change = 0;
        workout_stats.changes.strength_change = 0;

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
                ZoneName::Zone1 => (ZONE_1_STAMINA_POINTS_PER_MIN, ZONE_1_STRENGTH_POINTS_PER_MIN),
                ZoneName::Zone2 => (ZONE_2_STAMINA_POINTS_PER_MIN, ZONE_2_STRENGTH_POINTS_PER_MIN),
                ZoneName::Zone3 => (ZONE_3_STAMINA_POINTS_PER_MIN, ZONE_3_STRENGTH_POINTS_PER_MIN),
                ZoneName::Zone4 => (ZONE_4_STAMINA_POINTS_PER_MIN, ZONE_4_STRENGTH_POINTS_PER_MIN),
                ZoneName::Zone5 => (ZONE_5_STAMINA_POINTS_PER_MIN, ZONE_5_STRENGTH_POINTS_PER_MIN),
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
                zone: format!("{:?}", zone),
                minutes: *duration_minutes,
                stamina_gained: zone_stamina,
                strength_gained: zone_strength,
                hr_min,
                hr_max,
            });
        }

        workout_stats.changes.stamina_change = total_stamina as i32;
        workout_stats.changes.strength_change = total_strength as i32;
        workout_stats.zone_breakdown = Some(zone_breakdown);
        workout_stats
    }
}