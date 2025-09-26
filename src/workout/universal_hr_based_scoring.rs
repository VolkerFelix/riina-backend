use std::io::{Error, ErrorKind};
use chrono::Duration;

use crate::models::workout_data::{WorkoutStats, HeartRateData};
use crate::models::health::{UserHealthProfile, TrainingZones, TrainingZoneName};
use crate::game::stats_calculator::ScoringMethod;

const P_VT0: f32 = 0.4;
const P_VT1: f32 = 0.72;
const P_VT2: f32 = 0.88;

struct ZoneScore {
    duration: Duration,
    points: i32,
}

impl ZoneScore {
    pub fn new() -> Self {
        Self { duration: Duration::seconds(0), points: 0 }
    }
}

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

    // Go through heart rate data and calculate time spent in each zone + intensity

    Ok(WorkoutStats::new())
}

fn calculate_score_from_training_zones(training_zones: TrainingZones, hr_data: Vec<HeartRateData>) -> Result<WorkoutStats, Error> {
    let mut workout_stats = WorkoutStats::new();
    let mut rest = ZoneScore::new();
    let mut easy = ZoneScore::new();
    let mut moderate = ZoneScore::new();
    let mut hard = ZoneScore::new();

    for window in hr_data.windows(2) {
        let current_hr_sample = &window[0];
        let next_hr_sample = &window[1];
        let (current_zone, current_intensity) = match training_zones.get_zone_name_and_intensity(current_hr_sample.heart_rate) {
            Some((zone, intensity)) => (zone, intensity),
            None => {
                tracing::error!("No training rate zone found for heart rate {}", current_hr_sample.heart_rate);
                return Err(Error::new(ErrorKind::InvalidData, "No training rate zone found for heart rate"));
            }
        };
        let (next_zone, _next_intensity) = match training_zones.get_zone_name_and_intensity(next_hr_sample.heart_rate) {
            Some((zone, intensity)) => (zone, intensity),
            None => {
                tracing::error!("No training rate zone found for heart rate {}", next_hr_sample.heart_rate);
                return Err(Error::new(ErrorKind::InvalidData, "No training rate zone found for heart rate"));
            }
        };
        if current_zone == next_zone {
            // Same zone, add duration to the zone
            let duration = next_hr_sample.timestamp.signed_duration_since(current_hr_sample.timestamp);
            let duration_secs = duration.num_seconds() as f32;
            
            match current_zone {
                TrainingZoneName::REST => {
                    rest.duration += duration;
                    rest.points += (duration_secs * current_intensity) as i32;
                }
                TrainingZoneName::EASY => {
                    easy.duration += duration;
                    easy.points += (duration_secs * current_intensity) as i32;
                }
                TrainingZoneName::MODERATE => {
                    moderate.duration += duration;
                    moderate.points += (duration_secs * current_intensity) as i32;
                }
                TrainingZoneName::HARD => {
                    hard.duration += duration;
                    hard.points += (duration_secs * current_intensity) as i32;
                }
            }
        }

    }

    workout_stats.changes.stamina_change =  moderate.points + hard.points;
    workout_stats.changes.strength_change = rest.points + easy.points;

    Ok(workout_stats)
}