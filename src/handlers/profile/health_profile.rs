use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;

use crate::middleware::auth::Claims;
use crate::models::{
    profile::{HealthProfileResponse, UpdateHealthProfileRequest},
    health::Gender,
};
use crate::utils::health_calculations::calc_max_heart_rate;
use crate::db::health_data::update_max_heart_rate_and_vt_thresholds;

#[derive(Debug, Deserialize, Serialize)]
pub struct HealthProfileQuery {
    pub user_id: Option<String>,
}

#[tracing::instrument(
    name = "Get health profile",
    skip(pool, claims, query),
    fields(username = %claims.username)
)]
pub async fn get_health_profile(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    query: web::Query<HealthProfileQuery>
) -> HttpResponse {
    // Check if a user_id query parameter was provided
    let target_user_id = if let Some(user_id_str) = &query.user_id {
        // Requesting another user's health profile (for viewing their workout's heart rate zones)
        match Uuid::parse_str(user_id_str) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("Failed to parse user_id query parameter: {}", e);
                return HttpResponse::BadRequest().json(json!({
                    "error": "Invalid user_id parameter"
                }));
            }
        }
    } else {
        // Default: get the current user's own health profile
        let Some(id) = claims.user_id() else {
            tracing::error!("Invalid user ID in claims");
            return HttpResponse::BadRequest().json(json!({
                "error": "Invalid user ID"
            }));
        };
        id
    };

    // For privacy, only return VT thresholds and heart rate zones (not sensitive data like weight/height)
    // when fetching another user's profile
    let is_own_profile = claims.user_id().map(|id| id == target_user_id).unwrap_or(false);

    match sqlx::query_as!(
        HealthProfileResponse,
        r#"
        SELECT id, user_id, age, gender, resting_heart_rate, max_heart_rate,
               vt_off_threshold, vt0_threshold, vt1_threshold, vt2_threshold, weight, height, last_updated
        FROM user_health_profiles
        WHERE user_id = $1
        "#,
        target_user_id
    )
    .fetch_optional(&**pool)
    .await
    {
        Ok(Some(mut profile)) => {
            // For other users' profiles, redact sensitive personal data
            if !is_own_profile {
                profile.weight = None;
                profile.height = None;
                profile.age = None;
            }

            tracing::info!("Successfully retrieved health profile for user: {} (own profile: {})",
                          target_user_id, is_own_profile);
            HttpResponse::Ok().json(json!({
                "success": true,
                "data": profile
            }))
        }
        Ok(None) => {
            tracing::info!("No health profile found for user: {}", target_user_id);
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
    let Some(user_id) = claims.user_id() else {
        tracing::error!("Invalid user ID in claims");
        return HttpResponse::BadRequest().json(json!({
            "error": "Invalid user ID"
        }));
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
            if let Some(age) = profile_record.age {
                let gender = match profile_data.gender.as_deref() {
                    Some("male") | Some("m") => Gender::Male,
                    Some("female") | Some("f") => Gender::Female,
                    _ => Gender::Other,
                };
                let max_heart_rate = calc_max_heart_rate(age, gender);

                match update_max_heart_rate_and_vt_thresholds(
                    &pool,
                    user_id,
                    max_heart_rate,
                    profile_record.resting_heart_rate,
                ).await {
                    Ok(_) => {
                        tracing::info!("Successfully calculated and stored VT thresholds for user: {}", claims.username);
                    }
                    Err(e) => {
                        tracing::error!("Failed to update VT thresholds: {}", e);
                        // Continue execution - thresholds are optional
                    }
                }
            }
            
            // Fetch and return the updated profile
            match sqlx::query_as!(
                HealthProfileResponse,
                r#"
                SELECT id, user_id, age, gender, resting_heart_rate, max_heart_rate,
                       vt_off_threshold, vt0_threshold, vt1_threshold, vt2_threshold, weight, height, last_updated
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