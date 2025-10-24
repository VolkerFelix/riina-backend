use sqlx::{Pool, Postgres, Error};
use uuid::Uuid;

use crate::models::health::{UserHealthProfile, Gender, HeartRateZones, HeartRateZoneName};

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
                resting_heart_rate: Some(60),
                max_heart_rate: None,
                stored_heart_rate_zones: None,
            })
        }
    }
}

/// Update max heart rate and recalculate/store all heart rate zones and VT thresholds
pub async fn update_max_heart_rate_and_zones(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    new_max_hr: i32,
    age: i32,
    gender: Gender,
    resting_hr: i32,
) -> Result<HeartRateZones, Error> {
    use crate::workout::universal_hr_based_scoring::{P_VT0, P_VT1, P_VT2};

    // Calculate heart rate reserve and zones
    let hr_reserve = new_max_hr - resting_hr;
    let zones = HeartRateZones::new(age, gender, resting_hr);

    // Calculate VT thresholds
    let vt0_threshold = resting_hr + (hr_reserve as f32 * P_VT0) as i32;
    let vt1_threshold = resting_hr + (hr_reserve as f32 * P_VT1) as i32;
    let vt2_threshold = resting_hr + (hr_reserve as f32 * P_VT2) as i32;

    // Update in database
    sqlx::query!(
        r#"
        UPDATE user_health_profiles
        SET max_heart_rate = $1,
            hr_zone_1_max = $2,
            hr_zone_2_max = $3,
            hr_zone_3_max = $4,
            hr_zone_4_max = $5,
            hr_zone_5_max = $6,
            vt0_threshold = $7,
            vt1_threshold = $8,
            vt2_threshold = $9,
            last_updated = NOW()
        WHERE user_id = $10
        "#,
        new_max_hr,
        zones.zones.get(&HeartRateZoneName::Zone1).map(|z| z.high),
        zones.zones.get(&HeartRateZoneName::Zone2).map(|z| z.high),
        zones.zones.get(&HeartRateZoneName::Zone3).map(|z| z.high),
        zones.zones.get(&HeartRateZoneName::Zone4).map(|z| z.high),
        zones.zones.get(&HeartRateZoneName::Zone5).map(|z| z.high),
        vt0_threshold,
        vt1_threshold,
        vt2_threshold,
        user_id
    )
    .execute(pool)
    .await?;

    Ok(zones)
}