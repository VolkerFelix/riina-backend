use std::io::{Error, ErrorKind};

use crate::models::workout_data::{WorkoutStats, HeartRateData, ZoneBreakdown};
use crate::models::health::{UserHealthProfile, TrainingZones, TrainingZoneName};
use crate::game::stats_calculator::ScoringMethod;

pub const P_VT0: f32 = 0.35;
pub const P_VT1: f32 = 0.65;
pub const P_VT2: f32 = 0.8;

#[derive(Debug)]
struct ZoneScore {
    duration_mins: f32,
    points: f32,
}

impl ZoneScore {
    pub fn new() -> Self {
        Self { duration_mins: 0.0, points: 0.0 }
    }
}

pub struct UniversalHRBasedScoring;

impl ScoringMethod for UniversalHRBasedScoring {
    fn calculate_stats(&self, user_health_profile: UserHealthProfile, hr_data: Vec<HeartRateData>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<WorkoutStats, Error>> + Send + 'static>> {
        Box::pin(calculate_stats_universal_hr_based(user_health_profile, hr_data))
    }
}

async fn calculate_stats_universal_hr_based(user_health_profile: UserHealthProfile, hr_data: Vec<HeartRateData>) -> Result<WorkoutStats, Error> {
    if hr_data.is_empty() {
        return Ok(WorkoutStats::new());
    }
    // Calculate training zones
    let hr_max = user_health_profile.max_heart_rate.unwrap_or(300);
    let hr_rest = user_health_profile.resting_heart_rate.unwrap_or(60);
    let hr_reserve = hr_max - hr_rest;
    let training_zones = TrainingZones::new(hr_rest, hr_reserve, P_VT0, P_VT1, P_VT2);

    calculate_score_from_training_zones(training_zones, hr_data)
}

fn calculate_score_from_training_zones(training_zones: TrainingZones, hr_data: Vec<HeartRateData>) -> Result<WorkoutStats, Error> {
    let mut workout_stats = WorkoutStats::new();
    let mut rest = ZoneBreakdown::new(TrainingZoneName::REST.to_string());
    let mut easy = ZoneBreakdown::new(TrainingZoneName::EASY.to_string());
    let mut moderate = ZoneBreakdown::new(TrainingZoneName::MODERATE.to_string());
    let mut hard = ZoneBreakdown::new(TrainingZoneName::HARD.to_string());
    let mut points = 0.0;

    tracing::info!("📊 Processing {} heart rate data points", hr_data.len());

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
        let (_next_zone, _next_intensity) = match training_zones.get_zone_name_and_intensity(next_hr_sample.heart_rate) {
            Some((zone, intensity)) => (zone, intensity),
            None => {
                tracing::error!("No training rate zone found for heart rate {}", next_hr_sample.heart_rate);
                return Err(Error::new(ErrorKind::InvalidData, "No training rate zone found for heart rate"));
            }
        };

        // Always account for the time interval, attributing it to the current zone
        let duration = next_hr_sample.timestamp.signed_duration_since(current_hr_sample.timestamp);
        let duration_mins = duration.num_seconds().abs() as f32 / 60.0;
        let points_for_this_interval = duration_mins * current_intensity;
        points += points_for_this_interval;

        match current_zone {
            TrainingZoneName::REST => {
                rest.minutes += duration_mins;
                rest.stamina_gained += points_for_this_interval
            }
            TrainingZoneName::EASY => {
                easy.minutes += duration_mins;
                easy.stamina_gained += points_for_this_interval;
            }
            TrainingZoneName::MODERATE => {
                moderate.minutes += duration_mins;
                moderate.stamina_gained += points_for_this_interval;
            }
            TrainingZoneName::HARD => {
                hard.minutes += duration_mins;
                hard.stamina_gained += points_for_this_interval;
            }
        }

    }

    let zone_breakdown = vec![rest, easy, moderate, hard];

    workout_stats.changes.stamina_change = points;
    workout_stats.changes.strength_change = 0.0;
    workout_stats.zone_breakdown = Some(zone_breakdown);

    Ok(workout_stats)
}