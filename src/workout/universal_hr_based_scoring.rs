use std::io::Error;

use crate::models::workout_data::{WorkoutStats, HeartRateData};
use crate::models::health::{UserHealthProfile, TrainingZones};
use crate::game::stats_calculator::ScoringMethod;

const P_VT0: f32 = 0.4;
const P_VT1: f32 = 0.72;
const P_VT2: f32 = 0.88;

pub struct UniversalHRBasedScoring;

impl ScoringMethod for UniversalHRBasedScoring {
    fn calculate_stats(&self, user_health_profile: UserHealthProfile, hr_data: Vec<HeartRateData>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<WorkoutStats, Error>> + Send + 'static>> {
        Box::pin(calculate_stats_universal_hr_based(user_health_profile, hr_data))
    }
}

async fn calculate_stats_universal_hr_based(user_health_profile: UserHealthProfile, hr_data: Vec<HeartRateData>) -> Result<WorkoutStats, Error> {
    // Calculate training zones
    let hr_max = user_health_profile.max_heart_rate.unwrap_or(300);
    let hr_rest = user_health_profile.resting_heart_rate.unwrap_or(60);
    let hr_reserve = hr_max - hr_rest;
    let training_zones = TrainingZones::new(hr_rest, hr_reserve, P_VT0, P_VT1, P_VT2);

    Ok(WorkoutStats::new())
}