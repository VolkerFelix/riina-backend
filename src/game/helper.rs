use sqlx::{Pool, Postgres, Error};
use uuid::Uuid;

use crate::models::workout_data::{UserProfile, Gender, HeartRateZones};

pub async fn get_user_profile(pool: &Pool<Postgres>, user_id: Uuid) -> Result<UserProfile, Error> {
    tracing::info!("🔍 Fetching health profile for user: {}", user_id);
    let result = sqlx::query!(
        r#"
        SELECT age, gender, resting_heart_rate, max_heart_rate, 
               hr_zone_1_max, hr_zone_2_max, hr_zone_3_max, hr_zone_4_max, hr_zone_5_max
        FROM user_health_profiles 
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    match result {
        Some(row) => {
            let gender = match row.gender.as_deref() {
                Some("male") | Some("m") => Gender::Male,
                Some("female") | Some("f") => Gender::Female,
                _ => Gender::Other,
            };

            let resting_heart_rate = row.resting_heart_rate.unwrap_or(60);
            
            // Check if all heart rate zones are stored in the database
            let stored_heart_rate_zones = if let (Some(zone_1_max), Some(zone_2_max), Some(zone_3_max), Some(zone_4_max), Some(zone_5_max)) = 
                (row.hr_zone_1_max, row.hr_zone_2_max, row.hr_zone_3_max, row.hr_zone_4_max, row.hr_zone_5_max) {
                Some(HeartRateZones::from_stored_zones(
                    resting_heart_rate,
                    zone_1_max,
                    zone_2_max,
                    zone_3_max,
                    zone_4_max,
                    zone_5_max,
                ))
            } else {
                None
            };

            let profile = UserProfile {
                age: row.age.unwrap_or(30), // Default age if not provided
                gender,
                resting_heart_rate: Some(resting_heart_rate),
                max_heart_rate: row.max_heart_rate,
                stored_heart_rate_zones,
            };
            
            tracing::info!("✅ Found health profile: age={}, gender={:?}, resting_hr={:?}, stored_zones={}", 
                profile.age, profile.gender, profile.resting_heart_rate, profile.stored_heart_rate_zones.is_some());
            
            Ok(profile)
        }
        None => {
            tracing::warn!("⚠️ No health profile found for user, using defaults");
            // Default profile if user data not found
            Ok(UserProfile {
                age: 30,
                gender: Gender::Other,
                resting_heart_rate: None,
                max_heart_rate: None,
                stored_heart_rate_zones: None,
            })
        }
    }
}

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