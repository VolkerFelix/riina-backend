use actix_web::{web, HttpResponse};
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;

use crate::middleware::auth::Claims;
use crate::models::profile::{HealthProfileResponse, UpdateHealthProfileRequest};

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
        SELECT id, user_id, age, gender, resting_heart_rate, weight, height, last_updated
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
        if age < 10 || age > 120 {
            return HttpResponse::BadRequest().json(json!({
                "error": "Age must be between 10 and 120"
            }));
        }
    }

    if let Some(rhr) = profile_data.resting_heart_rate {
        if rhr < 30 || rhr > 120 {
            return HttpResponse::BadRequest().json(json!({
                "error": "Resting heart rate must be between 30 and 120 BPM"
            }));
        }
    }

    if let Some(weight) = profile_data.weight {
        if weight < 20.0 || weight > 300.0 {
            return HttpResponse::BadRequest().json(json!({
                "error": "Weight must be between 20 and 300 kg"
            }));
        }
    }

    if let Some(height) = profile_data.height {
        if height < 100.0 || height > 250.0 {
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
        RETURNING id
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
        Ok(_) => {
            tracing::info!("Successfully updated health profile for user: {}", claims.username);
            
            // Fetch and return the updated profile
            match sqlx::query_as!(
                HealthProfileResponse,
                r#"
                SELECT id, user_id, age, gender, resting_heart_rate, weight, height, last_updated
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