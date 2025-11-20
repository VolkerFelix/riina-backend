use sqlx::{Pool, Postgres, Error};
use uuid::Uuid;

use crate::models::health::{UserHealthProfile, Gender};
use crate::workout::universal_hr_based_scoring::{P_VT0, P_VT1, P_VT2};

pub async fn get_user_health_profile_details(pool: &Pool<Postgres>, user_id: Uuid) -> Result<UserHealthProfile, Error> {
    tracing::info!("🔍 Fetching health profile for user: {}", user_id);
    let result = sqlx::query!(
        r#"
        SELECT age, gender, resting_heart_rate, max_heart_rate
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

            let profile = UserHealthProfile {
                age: row.age.unwrap_or(30), // Default age if not provided
                gender,
                resting_heart_rate: row.resting_heart_rate,
                max_heart_rate: row.max_heart_rate,
            };

            tracing::info!("✅ Found health profile: age={}, gender={:?}, resting_hr={}, max_hr={}",
                profile.age, profile.gender, profile.resting_heart_rate, profile.max_heart_rate);

            Ok(profile)
        }
        None => {
            tracing::warn!("⚠️ No health profile found for user, using defaults");
            // Default profile if user data not found
            Ok(UserHealthProfile {
                age: 30,
                gender: Gender::Other,
                resting_heart_rate: 65,
                max_heart_rate: 190,
            })
        }
    }
}

/// Update max heart rate and calculate VT thresholds
pub async fn update_max_heart_rate_and_vt_thresholds(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    new_max_hr: i32,
    resting_hr: i32,
) -> Result<(), Error> {
    // Calculate heart rate reserve
    let hr_reserve = new_max_hr - resting_hr;

    // Calculate VT thresholds
    let vt0_threshold = resting_hr + (hr_reserve as f32 * P_VT0) as i32;
    let vt1_threshold = resting_hr + (hr_reserve as f32 * P_VT1) as i32;
    let vt2_threshold = resting_hr + (hr_reserve as f32 * P_VT2) as i32;

    // Update in database
    sqlx::query!(
        r#"
        UPDATE user_health_profiles
        SET max_heart_rate = $1,
            vt0_threshold = $2,
            vt1_threshold = $3,
            vt2_threshold = $4,
            last_updated = NOW()
        WHERE user_id = $5
        "#,
        new_max_hr,
        vt0_threshold,
        vt1_threshold,
        vt2_threshold,
        user_id
    )
    .execute(pool)
    .await?;

    Ok(())
}