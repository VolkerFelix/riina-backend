use sqlx::{Pool, Postgres, Error};
use uuid::Uuid;

use crate::models::health::{UserHealthProfile, Gender, HeartRateZones};

pub async fn get_user_health_profile_details(pool: &Pool<Postgres>, user_id: Uuid) -> Result<UserHealthProfile, Error> {
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
                    zone_1_max,
                    zone_2_max,
                    zone_3_max,
                    zone_4_max,
                    zone_5_max,
                ))
            } else {
                None
            };

            let profile = UserHealthProfile {
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
            Ok(UserHealthProfile {
                age: 30,
                gender: Gender::Other,
                resting_heart_rate: None,
                max_heart_rate: None,
                stored_heart_rate_zones: None,
            })
        }
    }
}