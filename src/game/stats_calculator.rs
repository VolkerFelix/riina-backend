use crate::models::health_data::{HealthDataSyncRequest, AdditionalMetrics, SleepData};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatChanges {
    pub stamina_change: i32,
    pub strength_change: i32,
    pub wisdom_change: i32,
    pub mana_change: i32,
    pub experience_change: i64,
    pub reasoning: Vec<String>, // Explain why stats changed
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GameStats {
    pub stamina: u32,
    pub strength: u32,
    pub wisdom: u32,
    pub mana: u32,
    pub experience_points: u64,
    pub level: u32,
}

pub struct StatCalculator;

impl StatCalculator {
    /// Calculate stat changes based on uploaded health data
    pub fn calculate_stat_changes(health_data: &HealthDataSyncRequest) -> StatChanges {
        let mut changes = StatChanges {
            stamina_change: 0,
            strength_change: 0,
            wisdom_change: 0,
            mana_change: 0,
            experience_change: 0,
            reasoning: Vec::new(),
        };

        // 🚶‍♂️ STEPS → STAMINA & EXPERIENCE
        if let Some(steps) = health_data.steps {
            let stamina_gain = Self::calculate_stamina_from_steps(steps);
            let exp_gain = Self::calculate_experience_from_steps(steps);
            
            if stamina_gain > 0 {
                changes.stamina_change += stamina_gain;
                changes.experience_change += exp_gain;
                changes.reasoning.push(format!("🚶‍♂️ {} steps → +{} Stamina, +{} XP", 
                    steps, stamina_gain, exp_gain));
            }
        }

        // 💓 HEART RATE → STRENGTH & STAMINA
        if let Some(heart_rate) = health_data.heart_rate {
            let (strength_gain, stamina_gain) = Self::calculate_stats_from_heart_rate(heart_rate);
            
            if strength_gain > 0 || stamina_gain > 0 {
                changes.strength_change += strength_gain;
                changes.stamina_change += stamina_gain;
                changes.experience_change += (strength_gain + stamina_gain) as i64 * 5;
                changes.reasoning.push(format!("💓 {} BPM workout → +{} Strength, +{} Stamina", 
                    heart_rate as i32, strength_gain, stamina_gain));
            }
        }

        // 😴 SLEEP → MANA & WISDOM
        if let Some(sleep_data) = &health_data.sleep {
            let (mana_gain, wisdom_gain, exp_gain) = Self::calculate_stats_from_sleep(sleep_data);
            
            changes.mana_change += mana_gain;
            changes.wisdom_change += wisdom_gain;
            changes.experience_change += exp_gain;
            
            if mana_gain > 0 || wisdom_gain > 0 {
                changes.reasoning.push(format!("😴 {:.1}h sleep → +{} Mana, +{} Wisdom", 
                    sleep_data.total_sleep_hours, mana_gain, wisdom_gain));
            }
        }

        // 🔥 ACTIVE ENERGY → STRENGTH & EXPERIENCE
        if let Some(energy_burned) = health_data.active_energy_burned {
            let (strength_gain, exp_gain) = Self::calculate_stats_from_energy(energy_burned);
            
            if strength_gain > 0 {
                changes.strength_change += strength_gain;
                changes.experience_change += exp_gain;
                changes.reasoning.push(format!("🔥 {} cal burned → +{} Strength, +{} XP", 
                    energy_burned as i32, strength_gain, exp_gain));
            }
        }

        // 🧘‍♂️ ADDITIONAL METRICS → VARIOUS STATS
        if let Some(additional) = &health_data.additional_metrics {
            let additional_changes = Self::calculate_stats_from_additional_metrics(additional);
            
            changes.stamina_change += additional_changes.stamina_change;
            changes.strength_change += additional_changes.strength_change;
            changes.wisdom_change += additional_changes.wisdom_change;
            changes.mana_change += additional_changes.mana_change;
            changes.experience_change += additional_changes.experience_change;
            changes.reasoning.extend(additional_changes.reasoning);
        }

        // 🌟 BONUS: Daily activity bonus
        let activity_bonus = Self::calculate_daily_activity_bonus(&changes);
        if activity_bonus.experience_change > 0 {
            changes.experience_change += activity_bonus.experience_change;
            changes.reasoning.extend(activity_bonus.reasoning);
        }

        changes
    }

    /// Steps → Stamina (1 point per 1000 steps, max 10 per session)
    fn calculate_stamina_from_steps(steps: i32) -> i32 {
        if steps < 500 { return 0; } // Minimum threshold
        
        let base_gain = steps / 1000; // 1 point per 1000 steps
        let bonus = if steps >= 10000 { 2 } else { 0 }; // 10k step bonus
        
        (base_gain + bonus).min(10) // Cap at 10 points per session
    }

    /// Steps → Experience (5 XP per 1000 steps)
    fn calculate_experience_from_steps(steps: i32) -> i64 {
        if steps < 500 { return 0; }
        (steps as f64 / 1000.0 * 5.0) as i64
    }

    /// Heart Rate → Strength & Stamina (workout intensity-based)
    fn calculate_stats_from_heart_rate(heart_rate: f32) -> (i32, i32) {
        match heart_rate as i32 {
            0..=60 => (0, 0),           // Resting
            61..=100 => (1, 1),         // Light activity  
            101..=140 => (2, 2),        // Moderate activity
            141..=170 => (3, 3),        // Vigorous activity
            171.. => (4, 2),            // High intensity (more strength, less stamina)
            _ => (0, 0),
        }
    }

    /// Sleep → Mana & Wisdom (recovery and learning)
    fn calculate_stats_from_sleep(sleep_data: &SleepData) -> (i32, i32, i64) {
        let sleep_hours = sleep_data.total_sleep_hours;
        
        // Quality sleep → Mana (magical energy from rest)
        let mana_gain = match sleep_hours {
            h if h < 4.0 => -2,         // Sleep debt
            h if h < 6.0 => 0,          // Poor sleep
            h if h < 7.0 => 1,          // Decent sleep
            h if h < 8.0 => 3,          // Good sleep
            h if h < 9.0 => 4,          // Great sleep
            _ => 3,                     // Too much sleep
        }.max(0);

        // Sleep → Wisdom (learning consolidation)
        let wisdom_gain = if sleep_hours >= 7.0 && sleep_hours <= 9.0 { 2 } else { 0 };
        
        // Experience bonus for good sleep
        let exp_gain = (mana_gain + wisdom_gain) as i64 * 3;

        (mana_gain, wisdom_gain, exp_gain)
    }

    /// Active Energy → Strength & Experience
    fn calculate_stats_from_energy(energy_burned: f32) -> (i32, i64) {
        let calories = energy_burned as i32;
        
        let strength_gain = match calories {
            0..=100 => 0,
            101..=200 => 1,
            201..=350 => 2,
            351..=500 => 3,
            501.. => 4,
            _ => 0,
        };

        let exp_gain = (calories / 50) as i64; // 1 XP per 50 calories

        (strength_gain, exp_gain)
    }

    /// Additional Metrics → Various Stats
    fn calculate_stats_from_additional_metrics(metrics: &AdditionalMetrics) -> StatChanges {
        let mut changes = StatChanges {
            stamina_change: 0,
            strength_change: 0,
            wisdom_change: 0,
            mana_change: 0,
            experience_change: 0,
            reasoning: Vec::new(),
        };

        // Blood Oxygen → Stamina
        if let Some(blood_oxygen) = metrics.blood_oxygen {
            if blood_oxygen >= 95 {
                changes.stamina_change += 1;
                changes.reasoning.push(format!("🫁 {}% blood oxygen → +1 Stamina", blood_oxygen));
            }
        }

        // HRV → Mana (stress recovery indicator)
        if let Some(hrv) = metrics.heart_rate_variability {
            if hrv >= 30 { // Good HRV indicates good recovery
                changes.mana_change += 2;
                changes.reasoning.push(format!("❤️‍🩹 {} HRV → +2 Mana (good recovery)", hrv));
            }
        }

        // Low Stress → Wisdom
        if let Some(stress_level) = metrics.stress_level {
            if stress_level <= 30 { // Lower stress is better
                changes.wisdom_change += 1;
                changes.reasoning.push(format!("🧘‍♂️ Low stress ({}) → +1 Wisdom", stress_level));
            }
        }

        // Resting Heart Rate → Stamina (lower is better)
        if let Some(rhr) = metrics.rest_heart_rate {
            if rhr <= 60 { // Athlete-level resting HR
                changes.stamina_change += 2;
                changes.reasoning.push(format!("💚 {} RHR → +2 Stamina (excellent fitness)", rhr));
            }
        }

        changes
    }

    /// Daily Activity Bonus (reward consistent activity)
    fn calculate_daily_activity_bonus(base_changes: &StatChanges) -> StatChanges {
        let total_stat_gains = base_changes.stamina_change + 
                              base_changes.strength_change + 
                              base_changes.wisdom_change + 
                              base_changes.mana_change;

        let mut bonus = StatChanges {
            stamina_change: 0,
            strength_change: 0,
            wisdom_change: 0,
            mana_change: 0,
            experience_change: 0,
            reasoning: Vec::new(),
        };

        // Bonus for well-rounded activity (multiple stat types improved)
        if total_stat_gains >= 8 {
            bonus.experience_change = 50;
            bonus.reasoning.push("🌟 Well-rounded activity bonus → +50 XP".to_string());
        } else if total_stat_gains >= 5 {
            bonus.experience_change = 25;
            bonus.reasoning.push("⭐ Active day bonus → +25 XP".to_string());
        }

        bonus
    }

    /// Calculate level from experience points
    pub fn calculate_level_from_experience(experience: u64) -> u32 {
        // Level formula: Level = floor(sqrt(XP / 100))
        // Level 1: 100 XP, Level 2: 400 XP, Level 3: 900 XP, etc.
        ((experience as f64 / 100.0).sqrt().floor() as u32).max(1)
    }

    /// Calculate XP needed for next level
    pub fn experience_for_next_level(current_level: u32) -> u64 {
        let next_level = current_level + 1;
        (next_level * next_level * 100) as u64
    }

    /// Apply stat changes to current stats (with level-up detection)
    pub fn apply_stat_changes(current_stats: &GameStats, changes: &StatChanges) -> (GameStats, bool) {
        let new_experience = (current_stats.experience_points as i64 + changes.experience_change).max(0) as u64;
        let new_level = Self::calculate_level_from_experience(new_experience);
        let leveled_up = new_level > current_stats.level;

        let new_stats = GameStats {
            stamina: ((current_stats.stamina as i32 + changes.stamina_change).max(0) as u32).min(100),
            strength: ((current_stats.strength as i32 + changes.strength_change).max(0) as u32).min(100),
            wisdom: ((current_stats.wisdom as i32 + changes.wisdom_change).max(0) as u32).min(100),
            mana: ((current_stats.mana as i32 + changes.mana_change).max(0) as u32).min(100),
            experience_points: new_experience,
            level: new_level,
        };

        (new_stats, leveled_up)
    }
}