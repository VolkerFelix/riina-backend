use serde::{Serialize, Deserialize};
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::models::health_data::{HealthDataSyncRequest, HeartRateData, HeartRateZones};
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

        // Determine HRR zone based on heart rate percentage
        // Assuming max HR ~= 220 - age (using 180 as average for adults)
        let hr_percentage = (heart_rate / 180.0 * 100.0) as i32;
        
        // Calculate base gains from HRR zone
        let (stamina_gain, strength_gain, zone_name) = match hr_percentage {
            50..=60 => {
                // Zone 1: Active Recovery - minimal gains for consistency
                let base_gain = (calories / 100.0) as i32;
                (base_gain.min(1), 0, "Active Recovery")
            },
            61..=70 => {
                // Zone 2: Aerobic Base - primary stamina gains
                let stamina = (calories / 75.0) as i32;
                (stamina.min(4), 0, "Aerobic Base")
            },
            71..=80 => {
                // Zone 3: Aerobic - stamina + small strength
                let stamina = (calories / 80.0) as i32;
                let strength = (calories / 200.0) as i32;
                (stamina.min(3), strength.min(1), "Aerobic")
            },
            81..=90 => {
                // Zone 4: Lactate Threshold - balanced stamina + strength
                let stamina = (calories / 100.0) as i32;
                let strength = (calories / 120.0) as i32;
                (stamina.min(3), strength.min(2), "Lactate Threshold")
            },
            91..=220 => {
                // Zone 5: Neuromuscular Power - primary strength + XP bonus
                let stamina = (calories / 150.0) as i32;
                let strength = (calories / 80.0) as i32;
                (stamina.min(2), strength.min(4), "Neuromuscular Power")
            },
            _ => (0, 0, "Invalid Zone"),
        };

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