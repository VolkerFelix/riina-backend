use serde::{Serialize, Deserialize};
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::models::game::*;
use crate::models::workout_data::{WorkoutDataSyncRequest, HeartRateData, HeartRateZones, ZoneName};
use crate::game::helper::{get_user_profile, calc_max_heart_rate};
use crate::workout::workout_analyzer::WorkoutAnalyzer;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZoneBreakdown {
    pub zone: String,
    pub minutes: f32,
    pub stamina_gained: i32,
    pub strength_gained: i32,
    pub hr_min: Option<i32>, // Lower heart rate limit for this zone
    pub hr_max: Option<i32>, // Upper heart rate limit for this zone
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatChanges {
    pub stamina_change: i32,
    pub strength_change: i32,
    pub reasoning: Vec<String>,
    pub zone_breakdown: Option<Vec<ZoneBreakdown>>,
}

pub struct StatCalculator;

impl StatCalculator {
    /// Calculate stat changes based on HRR zones from heart rate and calories
    pub async fn calculate_stat_changes(pool: &Pool<Postgres>, user_id: Uuid, workout_data: &WorkoutDataSyncRequest) -> StatChanges {
        let mut changes = StatChanges {
            stamina_change: 0,
            strength_change: 0,
            reasoning: Vec::new(),
            zone_breakdown: None,
        };

        if let Some(heart_rate) = &workout_data.heart_rate {
            let stats_changes = Self::calc_stats_hhr_based(pool, user_id, heart_rate).await;
            changes.stamina_change += stats_changes.stamina_change;
            changes.strength_change += stats_changes.strength_change;
            changes.zone_breakdown = stats_changes.zone_breakdown;
        }
        changes
    }

    /// Calculate base stats from HRR zones based on heart rate
    async fn calc_stats_hhr_based(pool: &Pool<Postgres>, user_id: Uuid, heart_rate: &Vec<HeartRateData>) -> StatChanges {
        let mut changes = StatChanges {
            stamina_change: 0,
            strength_change: 0,
            reasoning: Vec::new(),
            zone_breakdown: None,
        };

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
        
        tracing::info!("📊 Processing {} heart rate data points", heart_rate.len());
        if !heart_rate.is_empty() {
            let avg_hr: i32 = heart_rate.iter().map(|hr| hr.heart_rate).sum::<i32>() / heart_rate.len() as i32;
            tracing::info!("💗 Heart rate range: avg={:.1}, min={:.1}, max={:.1}", 
                avg_hr,
                heart_rate.iter().map(|hr| hr.heart_rate).fold(i32::MAX, i32::min),
                heart_rate.iter().map(|hr| hr.heart_rate).fold(0, i32::max)
            );
        }
        
        if let Some(workout_analysis) = WorkoutAnalyzer::new(heart_rate, &heart_rate_zones) {
            tracing::info!("✅ WorkoutAnalyzer created successfully");
            for (zone, minutes) in &workout_analysis.zone_durations {
                tracing::info!("📈 Zone {:?}: {:.1} minutes", zone, minutes);
            }
            let (points_changes, zone_breakdown) = Self::calc_points_and_breakdown_from_workout_analysis(&workout_analysis, &heart_rate_zones);
            changes.stamina_change += points_changes.stamina_change;
            changes.strength_change += points_changes.strength_change;
            changes.zone_breakdown = Some(zone_breakdown);

            // Add zone distribution info
            for (zone, minutes) in &workout_analysis.zone_durations {
                if *minutes > 0.5 { // Only show zones with significant time
                    changes.reasoning.push(format!(
                        "{:?}: {:.1} min", zone, minutes
                    ));
                }
            }

            changes.reasoning.push(format!(
                "Avg HR: {:.0} bpm, Peak HR: {:.0} bpm", 
                workout_analysis.avg_heart_rate, workout_analysis.peak_heart_rate
            ));
            
            tracing::info!("🎯 Final stat changes: stamina +{}, strength +{}", 
                changes.stamina_change, changes.strength_change);
        } else {
            tracing::error!("❌ WorkoutAnalyzer::new() returned None - no stats calculated");
        }

        changes
    }

    fn calc_points_and_breakdown_from_workout_analysis(workout_analysis: &WorkoutAnalyzer, heart_rate_zones: &HeartRateZones) -> (StatChanges, Vec<ZoneBreakdown>) {
        let mut changes = StatChanges {
            stamina_change: 0,
            strength_change: 0,
            reasoning: Vec::new(),
            zone_breakdown: None,
        };

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

        changes.stamina_change = total_stamina as i32;
        changes.strength_change = total_strength as i32;
        (changes, zone_breakdown)
    }
}