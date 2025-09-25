use std::collections::HashMap;
use std::f32::consts::E;

#[derive(Debug, Clone)]
pub struct ZoneRange {
    pub low: i32,
    pub high: i32,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum HeartRateZoneName {
    Zone1,
    Zone2,
    Zone3,
    Zone4,
    Zone5,
}

#[derive(Debug, Clone)]
pub struct HeartRateZones {
    pub zones: HashMap<HeartRateZoneName, ZoneRange>,
}

impl HeartRateZones {
    pub fn new(hhr: i32, resting_heart_rate: i32, max_heart_rate: i32) -> Self {
        let zone_1 = ZoneRange {
            low: 0, // Zone 1 starts from 0 bpm to capture all heart rates including below resting
            high: resting_heart_rate + (hhr as f32 * 0.6) as i32 - 1,
        };
        let zone_2 = ZoneRange {
            low: resting_heart_rate + (hhr as f32 * 0.6) as i32,
            high: resting_heart_rate + (hhr as f32 * 0.7) as i32 - 1,
        };
        let zone_3 = ZoneRange {
            low: resting_heart_rate + (hhr as f32 * 0.7) as i32,
            high: resting_heart_rate + (hhr as f32 * 0.8) as i32 - 1,
        };
        let zone_4 = ZoneRange {
            low: resting_heart_rate + (hhr as f32 * 0.8) as i32,
            high: resting_heart_rate + (hhr as f32 * 0.9) as i32 - 1,
        };
        let zone_5 = ZoneRange {
            low: resting_heart_rate + (hhr as f32 * 0.9) as i32,
            high: max_heart_rate,
        };
        Self {
            zones: HashMap::from([
                (HeartRateZoneName::Zone1, zone_1),
                (HeartRateZoneName::Zone2, zone_2),
                (HeartRateZoneName::Zone3, zone_3),
                (HeartRateZoneName::Zone4, zone_4),
                (HeartRateZoneName::Zone5, zone_5),
            ]),
        }
    }

    pub fn from_stored_zones(
        zone_1_max: i32,
        zone_2_max: i32,
        zone_3_max: i32,
        zone_4_max: i32,
        zone_5_max: i32,
    ) -> Self {
        let zone_1 = ZoneRange {
            low: 0, // Zone 1 starts from 0 bpm to capture all heart rates
            high: zone_1_max,
        };
        let zone_2 = ZoneRange {
            low: zone_1_max + 1,
            high: zone_2_max,
        };
        let zone_3 = ZoneRange {
            low: zone_2_max + 1,
            high: zone_3_max,
        };
        let zone_4 = ZoneRange {
            low: zone_3_max + 1,
            high: zone_4_max,
        };
        let zone_5 = ZoneRange {
            low: zone_4_max + 1,
            high: zone_5_max,
        };
        Self {
            zones: HashMap::from([
                (HeartRateZoneName::Zone1, zone_1),
                (HeartRateZoneName::Zone2, zone_2),
                (HeartRateZoneName::Zone3, zone_3),
                (HeartRateZoneName::Zone4, zone_4),
                (HeartRateZoneName::Zone5, zone_5),
            ]),
        }
    }

    pub fn get_zone(&self, heart_rate: f32) -> Option<HeartRateZoneName> {
        for (zone_name, zone_range) in &self.zones {
            if heart_rate >= zone_range.low as f32 && heart_rate <= zone_range.high as f32 {
                return Some(*zone_name);
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub enum Gender {
    Male,
    Female,
    Other, // Use male formulas as default
}

#[derive(Debug, Clone)]
pub struct UserHealthProfile {
    pub age: i32,
    pub gender: Gender,
    pub resting_heart_rate: Option<i32>,
    pub max_heart_rate: Option<i32>,
    pub stored_heart_rate_zones: Option<HeartRateZones>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum TrainingZoneName {
    REST,
    EASY,
    MODERATE,
    HARD
}

struct IntensityWeight {
    weighting_fn: Box<dyn Fn(f32) -> f32>,
}

impl std::fmt::Debug for IntensityWeight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IntensityWeight {{ weighting_fn: <closure> }}")
    }
}

#[derive(Debug)]
pub struct TrainingZones {
    pub zones: HashMap<TrainingZoneName, (ZoneRange, IntensityWeight)>,
}

impl TrainingZones {
    pub fn new(hr_rest: i32, hr_reserve: i32, p_vt0: f32, p_vt1: f32, p_vt2: f32) -> Self {
        let rest_zone = ZoneRange {
            low: 0,
            high: hr_rest + (hr_reserve as f32 * p_vt0) as i32 - 1,
        };
        let easy_zone = ZoneRange {
            low: hr_rest + (hr_reserve as f32 * p_vt0) as i32,
            high: hr_rest + (hr_reserve as f32 * p_vt1) as i32 - 1,
        };
        let moderate_zone = ZoneRange {
            low: hr_rest + (hr_reserve as f32 * p_vt1) as i32,
            high: hr_rest + (hr_reserve as f32 * p_vt2) as i32 - 1,
        };
        let hard_zone = ZoneRange {
            low: hr_rest + (hr_reserve as f32 * p_vt2) as i32,
            high: 300,
        };
        Self { zones: HashMap::from([
            (TrainingZoneName::REST, (rest_zone, IntensityWeight { weighting_fn: Box::new(|x| x * 0.0) })),
            (TrainingZoneName::EASY, (easy_zone, IntensityWeight { weighting_fn: Box::new(|x| x * 1.0) })),
            (TrainingZoneName::MODERATE, (moderate_zone, IntensityWeight { weighting_fn: Box::new(|x| x * 2.0) })),
            (TrainingZoneName::HARD, (hard_zone.clone(), IntensityWeight { weighting_fn: Box::new(move |hr| intensity_weight_heart_rate(hr, hard_zone.low as f32)) })),
        ]) }
    }
}

fn intensity_weight_heart_rate(hr: f32, vt2_high: f32) -> f32 {
    2.0 * E.powf(0.04 * (hr - vt2_high))
}