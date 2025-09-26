use crate::models::health::{Gender};

pub fn calc_max_heart_rate(age: i32, gender: Gender) -> i32 {
    match gender {
        Gender::Male => {
            if age >= 40 {
                (216.0 - (0.93 * age as f32)) as i32 // Research-based formula for men 40+
            } else {
                (208.0 - (0.7 * age as f32)) as i32 // General formula for younger men
            }
        }
        Gender::Female => {
            if age >= 40 {
                (200.0 - (0.67 * age as f32)) as i32 // Research-based formula for women 40+
            } else {
                (206.0 - (0.88 * age as f32)) as i32 // Adjusted formula for younger women
            }
        }
        Gender::Other => {
            (208.0 - (0.7 * age as f32)) as i32 // Use general formula as default
        }
    }
}