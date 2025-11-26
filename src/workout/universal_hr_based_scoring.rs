use std::io::{Error, ErrorKind};

use crate::models::workout_data::{WorkoutStats, HeartRateData, ZoneBreakdown, WorkoutType};
use crate::models::health::{UserHealthProfile, TrainingZones, TrainingZoneName};
use crate::game::stats_calculator::ScoringMethod;

pub const P_VT0: f32 = 0.35;
pub const P_VT1: f32 = 0.65;
pub const P_VT2: f32 = 0.8;
pub const STRENGTH_WORKOUT_MULTIPLIER: f32 = 1.5;

pub struct UniversalHRBasedScoring;

impl ScoringMethod for UniversalHRBasedScoring {
    fn calculate_stats(
        &self,
        user_health_profile: UserHealthProfile,
        hr_data: Vec<HeartRateData>,
        workout_type: WorkoutType
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<WorkoutStats, Error>> + Send + 'static>> {
        Box::pin(calculate_stats_universal_hr_based(user_health_profile, hr_data, workout_type))
    }
}

async fn calculate_stats_universal_hr_based(
    user_health_profile: UserHealthProfile,
    hr_data: Vec<HeartRateData>,
    workout_type: WorkoutType
) -> Result<WorkoutStats, Error> {
    if hr_data.is_empty() {
        return Ok(WorkoutStats::new());
    }
    // Calculate training zones
    let hr_max = user_health_profile.max_heart_rate;
    let hr_rest = user_health_profile.resting_heart_rate;
    let hr_reserve = hr_max - hr_rest;
    let training_zones = TrainingZones::new(hr_rest, hr_reserve, P_VT0, P_VT1, P_VT2);

    let mut workout_stats = calculate_score_from_training_zones(training_zones, hr_data)?;

    if workout_type == WorkoutType::Strength {
        workout_stats.changes.stamina_change *= STRENGTH_WORKOUT_MULTIPLIER;
        workout_stats.changes.strength_change *= STRENGTH_WORKOUT_MULTIPLIER;
    }

    Ok(workout_stats)
}

fn calculate_score_from_training_zones(training_zones: TrainingZones, hr_data: Vec<HeartRateData>) -> Result<WorkoutStats, Error> {
    let mut workout_stats = WorkoutStats::new();

    // Initialize zone breakdowns with HR ranges
    let rest_zone = training_zones.zones.get(&TrainingZoneName::REST).unwrap();
    let mut rest = ZoneBreakdown::new(TrainingZoneName::REST.to_string());
    rest.hr_min = Some(rest_zone.zone.low);
    rest.hr_max = Some(rest_zone.zone.high);

    let easy_zone = training_zones.zones.get(&TrainingZoneName::EASY).unwrap();
    let mut easy = ZoneBreakdown::new(TrainingZoneName::EASY.to_string());
    easy.hr_min = Some(easy_zone.zone.low);
    easy.hr_max = Some(easy_zone.zone.high);

    let moderate_zone = training_zones.zones.get(&TrainingZoneName::MODERATE).unwrap();
    let mut moderate = ZoneBreakdown::new(TrainingZoneName::MODERATE.to_string());
    moderate.hr_min = Some(moderate_zone.zone.low);
    moderate.hr_max = Some(moderate_zone.zone.high);

    let hard_zone = training_zones.zones.get(&TrainingZoneName::HARD).unwrap();
    let mut hard = ZoneBreakdown::new(TrainingZoneName::HARD.to_string());
    hard.hr_min = Some(hard_zone.zone.low);
    hard.hr_max = Some(hard_zone.zone.high);

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