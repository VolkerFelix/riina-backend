use sqlx::{Pool, Postgres, Error};
use uuid::Uuid;

use crate::models::health_data::{UserProfile, Gender};

pub async fn get_user_profile(pool: &Pool<Postgres>, user_id: Uuid) -> Result<UserProfile, Error> {
    let result = sqlx::query!(
        r#"
        SELECT age, gender, resting_heart_rate
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

            Ok(UserProfile {
                age: row.age.unwrap_or(30), // Default age if not provided
                gender,
                resting_heart_rate: row.resting_heart_rate,
            })
        }
        None => {
            // Default profile if user data not found
            Ok(UserProfile {
                age: 30,
                gender: Gender::Other,
                resting_heart_rate: None,
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