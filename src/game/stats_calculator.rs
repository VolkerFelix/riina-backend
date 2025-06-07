use serde::{Serialize, Deserialize};
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::models::game::*;
use crate::models::health_data::{HealthDataSyncRequest, HeartRateData, HeartRateZones, ZoneName};
use crate::game::helper::get_hhr_and_resting_hr;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatChanges {
    pub stamina_change: i32,
    pub strength_change: i32,
    pub experience_change: i64,
    pub reasoning: Vec<String>, // Explain why stats changed
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
            experience_change: 0,
            reasoning: Vec::new(),
        };

        if let Some(heart_rate) = &health_data.heart_rate {
            if let Some(calories) = health_data.active_energy_burned {
                let hrr_changes = Self::calculate_stats_from_hrr_zones(pool, user_id, heart_rate, calories).await;
                changes.stamina_change += hrr_changes.stamina_change;
                changes.strength_change += hrr_changes.strength_change;
                changes.experience_change += hrr_changes.experience_change;
                changes.reasoning.extend(hrr_changes.reasoning);
            }
        }
        changes
    }

    /// Calculate stats from HRR zones based on heart rate and calories burned
    async fn calculate_stats_from_hrr_zones(pool: &Pool<Postgres>, user_id: Uuid, heart_rate: &Vec<HeartRateData>, calories: f32) -> StatChanges {
        let mut changes = StatChanges {
            stamina_change: 0,
            strength_change: 0,
            experience_change: 0,
            reasoning: Vec::new(),
        };

        let (hhr, resting_heart_rate) = get_hhr_and_resting_hr(pool, user_id).await.unwrap();
        let heart_rate_zones = HeartRateZones::new(hhr, resting_heart_rate);
        
        // Go through each heart rate data point and determine the zone
        for heart_rate_data in heart_rate {
            let heart_rate_sample = heart_rate_data.heart_rate;
            let zone = heart_rate_zones.get_zone(heart_rate_sample);
            let stamina_gain = match zone {
                Some(ZoneName::Zone1) => ZONE_1_STAMINA_POINTS,
                Some(ZoneName::Zone2) => ZONE_2_STAMINA_POINTS,
                Some(ZoneName::Zone3) => ZONE_3_STAMINA_POINTS,
                Some(ZoneName::Zone4) => ZONE_4_STAMINA_POINTS,
                Some(ZoneName::Zone5) => ZONE_5_STAMINA_POINTS,
                None => 0,
            };
            let strength_gain = match zone {
                Some(ZoneName::Zone1) => ZONE_1_STRENGTH_POINTS,
                Some(ZoneName::Zone2) => ZONE_2_STRENGTH_POINTS,
                Some(ZoneName::Zone3) => ZONE_3_STRENGTH_POINTS,
                Some(ZoneName::Zone4) => ZONE_4_STRENGTH_POINTS,
                Some(ZoneName::Zone5) => ZONE_5_STRENGTH_POINTS,
                None => 0,
            };
            changes.stamina_change += stamina_gain;
            changes.strength_change += strength_gain;
            changes.experience_change += experience_gain;
            changes.reasoning.push(format!("💓 {}% HRR ({}) • {} cal → +{} Stamina, +{} Strength, +{} XP", hr_percentage, zone_name, calories as i32, stamina_gain, strength_gain, experience_gain));
        }

        changes.stamina_change = stamina_gain;
        changes.strength_change = strength_gain;
        
        // Calculate experience based on total gains
        changes.experience_change = (stamina_gain + strength_gain) as i64 * 10;
        
        // Add XP bonus for high intensity zones
        if hr_percentage >= 91 {
            changes.experience_change += 20; // Bonus for Zone 5
        }

        if stamina_gain > 0 || strength_gain > 0 {
            changes.reasoning.push(format!(
                "💓 {}% HRR ({}) • {} cal → +{} Stamina, +{} Strength, +{} XP",
                hr_percentage,
                zone_name,
                calories as i32,
                stamina_gain,
                strength_gain,
                changes.experience_change
            ));
        }

        changes
    }
}