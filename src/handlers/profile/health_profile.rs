use actix_web::{web, HttpResponse};
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;

use crate::middleware::auth::Claims;
use crate::models::{
    profile::{HealthProfileResponse, UpdateHealthProfileRequest},
    health::Gender,
};
use crate::utils::health_calculations::calc_max_heart_rate;
use crate::db::health_data::update_max_heart_rate_and_zones;

#[tracing::instrument(
    name = "Get health profile",
    skip(pool, claims),
    fields(username = %claims.username)
)]
pub async fn get_health_profile(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "error": "Invalid user ID"
            }));
        }
    };

    match sqlx::query_as!(
        HealthProfileResponse,
        r#"
        SELECT id, user_id, age, gender, resting_heart_rate, max_heart_rate,
               vt0_threshold, vt1_threshold, vt2_threshold, weight, height, last_updated
        FROM user_health_profiles
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(&**pool)
    .await
    {
        Ok(Some(profile)) => {
            tracing::info!("Successfully retrieved health profile for user: {}", claims.username);
            HttpResponse::Ok().json(json!({
                "success": true,
                "data": profile
            }))
        }
        Ok(None) => {
            tracing::info!("No health profile found for user: {}", claims.username);
            HttpResponse::NotFound().json(json!({
                "error": "Health profile not found"
            }))
        }
        Err(e) => {
            tracing::error!("Database error fetching health profile: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "error": "Failed to fetch health profile"
            }))
        }
    }
}

#[tracing::instrument(
    name = "Update health profile",
    skip(pool, claims, profile_data),
    fields(username = %claims.username)
)]
pub async fn update_health_profile(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    profile_data: web::Json<UpdateHealthProfileRequest>
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "error": "Invalid user ID"
            }));
        }
    };
    tracing::info!("Updating health profile for user: {}", user_id);

    // Validate input data
    if let Some(age) = profile_data.age {
        if !(10..=120).contains(&age) {
            return HttpResponse::BadRequest().json(json!({
                "error": "Age must be between 10 and 120"
            }));
        }
    }

    if let Some(rhr) = profile_data.resting_heart_rate {
        if !(30..=120).contains(&rhr) {
            return HttpResponse::BadRequest().json(json!({
                "error": "Resting heart rate must be between 30 and 120 BPM"
            }));
        }
    }

    if let Some(weight) = profile_data.weight {
        if !(20.0..=300.0).contains(&weight) {
            return HttpResponse::BadRequest().json(json!({
                "error": "Weight must be between 20 and 300 kg"
            }));
        }
    }

    if let Some(height) = profile_data.height {
        if !(100.0..=250.0).contains(&height) {
            return HttpResponse::BadRequest().json(json!({
                "error": "Height must be between 100 and 250 cm"
            }));
        }
    }

    let result = sqlx::query!(
        r#"
        INSERT INTO user_health_profiles (user_id, age, gender, resting_heart_rate, weight, height, last_updated)
        VALUES ($1, $2, $3, $4, $5, $6, NOW())
        ON CONFLICT (user_id) 
        DO UPDATE SET 
            age = COALESCE($2, user_health_profiles.age),
            gender = COALESCE($3, user_health_profiles.gender),
            resting_heart_rate = COALESCE($4, user_health_profiles.resting_heart_rate),
            weight = COALESCE($5, user_health_profiles.weight),
            height = COALESCE($6, user_health_profiles.height),
            last_updated = NOW()
        RETURNING id, age, resting_heart_rate
        "#,
        user_id,
        profile_data.age,
        profile_data.gender.as_deref(),
        profile_data.resting_heart_rate,
        profile_data.weight,
        profile_data.height
    )
    .fetch_one(&**pool)
    .await;

    match result {
        Ok(profile_record) => {
            tracing::info!("Successfully updated health profile for user: {}", claims.username);
            
            // Calculate and store heart rate zones if we have age and resting heart rate
            if let (Some(age), Some(resting_heart_rate)) = (profile_record.age, profile_record.resting_heart_rate) {
                let gender = match profile_data.gender.as_deref() {
                    Some("male") | Some("m") => Gender::Male,
                    Some("female") | Some("f") => Gender::Female,
                    _ => Gender::Other,
                };
                let max_heart_rate = calc_max_heart_rate(age, gender);

                // Use the centralized function to update max HR and zones
                if let Err(e) = update_max_heart_rate_and_zones(
                    &pool,
                    user_id,
                    max_heart_rate,
                    age,
                    gender,
                    resting_heart_rate,
                ).await {
                    tracing::error!("Failed to update heart rate zones: {}", e);
                    // Continue execution - zones are optional
                } else {
                    tracing::info!("Successfully calculated and stored heart rate zones for user: {}", claims.username);
                }
            }
            
            // Fetch and return the updated profile
            match sqlx::query_as!(
                HealthProfileResponse,
                r#"
                SELECT id, user_id, age, gender, resting_heart_rate, max_heart_rate,
                       vt0_threshold, vt1_threshold, vt2_threshold, weight, height, last_updated
                FROM user_health_profiles
                WHERE user_id = $1
                "#,
                user_id
            )
            .fetch_one(&**pool)
            .await
            {
                Ok(profile) => HttpResponse::Ok().json(json!({
                    "success": true,
                    "data": profile,
                    "message": "Health profile updated successfully"
                })),
                Err(e) => {
                    tracing::error!("Failed to fetch updated profile: {}", e);
                    HttpResponse::InternalServerError().json(json!({
                        "error": "Profile updated but failed to retrieve updated data"
                    }))
                }
            }
        }
        Err(e) => {
            tracing::error!("Database error updating health profile: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "error": "Failed to update health profile"
            }))
        }
    }
}