use serde::{Serialize, Deserialize};
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::models::game::*;
use crate::models::health_data::{HealthDataSyncRequest, HeartRateData, HeartRateZones, ZoneName};
use crate::game::helper::{get_user_profile, calc_max_heart_rate};
use crate::workout::workout_analyzer::WorkoutAnalyzer;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatChanges {
    pub stamina_change: i32,
    pub strength_change: i32,
    pub reasoning: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GameStats {
    pub stamina: u32,
    pub strength: u32,
    pub experience_points: u64,
    pub level: u32,
}

pub struct StatCalculator;

impl StatCalculator {
    /// Calculate stat changes based on HRR zones from heart rate and calories
    pub async fn calculate_stat_changes(pool: &Pool<Postgres>, user_id: Uuid, health_data: &HealthDataSyncRequest) -> StatChanges {
        let mut changes = StatChanges {
            stamina_change: 0,
            strength_change: 0,
            reasoning: Vec::new(),
        };

        if let Some(heart_rate) = &health_data.heart_rate {
            let stats_changes = Self::calc_stats_hhr_based(pool, user_id, heart_rate).await;
            changes.stamina_change += stats_changes.stamina_change;
            changes.strength_change += stats_changes.strength_change;
        }
        changes
    }

    /// Calculate base stats from HRR zones based on heart rate
    async fn calc_stats_hhr_based(pool: &Pool<Postgres>, user_id: Uuid, heart_rate: &Vec<HeartRateData>) -> StatChanges {
        let mut changes = StatChanges {
            stamina_change: 0,
            strength_change: 0,
            reasoning: Vec::new(),
        };

        let user_profile = get_user_profile(pool, user_id).await.unwrap();
        let max_heart_rate = calc_max_heart_rate(user_profile.age, user_profile.gender);
        let hrr = max_heart_rate - user_profile.resting_heart_rate.unwrap_or(0);
        let heart_rate_zones = HeartRateZones::new(hrr, user_profile.resting_heart_rate.unwrap_or(0), max_heart_rate);
        if let Some(workout_analysis) = WorkoutAnalyzer::new(heart_rate, &heart_rate_zones) {
            let points_changes = Self::calc_points_from_workout_analysis(&workout_analysis);
            changes.stamina_change += points_changes.stamina_change;
            changes.strength_change += points_changes.strength_change;

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
        }

        changes
    }

    fn calc_points_from_workout_analysis(workout_analysis: &WorkoutAnalyzer) -> StatChanges {
        let mut changes = StatChanges {
            stamina_change: 0,
            strength_change: 0,
            reasoning: Vec::new(),
        };

        let mut total_stamina = 0.0;
        let mut total_strength = 0.0;

        for (zone, duration_minutes) in &workout_analysis.zone_durations {
            let (stamina_per_min, strength_per_min) = match zone {
                ZoneName::Zone1 => (ZONE_1_STAMINA_POINTS_PER_MIN, ZONE_1_STRENGTH_POINTS_PER_MIN),
                ZoneName::Zone2 => (ZONE_2_STAMINA_POINTS_PER_MIN, ZONE_2_STRENGTH_POINTS_PER_MIN),
                ZoneName::Zone3 => (ZONE_3_STAMINA_POINTS_PER_MIN, ZONE_3_STRENGTH_POINTS_PER_MIN),
                ZoneName::Zone4 => (ZONE_4_STAMINA_POINTS_PER_MIN, ZONE_4_STRENGTH_POINTS_PER_MIN),
                ZoneName::Zone5 => (ZONE_5_STAMINA_POINTS_PER_MIN, ZONE_5_STRENGTH_POINTS_PER_MIN),
            };
            
            total_stamina += duration_minutes * stamina_per_min as f32;
            total_strength += duration_minutes * strength_per_min as f32;
        }

        changes.stamina_change = total_stamina as i32;
        changes.strength_change = total_strength as i32;
        changes
    }
}