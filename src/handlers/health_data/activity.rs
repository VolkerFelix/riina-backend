use actix_web::{web, HttpResponse};
use serde_json::json;
use uuid::Uuid;
use sqlx::PgPool;
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};

use crate::middleware::auth::Claims;
use crate::models::health_data::{ActivitySummaryResponse, WeeklyStats, MonthlyTrend};

#[tracing::instrument(
    name = "Get user activity summary",
    skip(pool, claims),
    fields(username = %claims.username)
)]
pub async fn get_activity_summary(
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

    // Calculate date ranges
    let now = Utc::now();
    let week_ago = now - Duration::days(7);
    let month_ago = now - Duration::days(30);

    // Get recent workout count (last 7 days)
    let recent_workouts = match sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM health_data 
        WHERE user_id = $1 
        AND created_at >= $2
        AND active_energy_burned > 200  -- Only count significant activity
        "#,
        user_id,
        week_ago
    )
    .fetch_one(&**pool)
    .await
    {
        Ok(row) => row.count.unwrap_or(0) as i32,
        Err(e) => {
            tracing::error!("Failed to get recent workouts count: {}", e);
            0
        }
    };

    // Get total sessions (all time)
    let total_sessions = match sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM health_data 
        WHERE user_id = $1
        AND active_energy_burned > 100  -- Any recorded activity
        "#,
        user_id
    )
    .fetch_one(&**pool)
    .await
    {
        Ok(row) => row.count.unwrap_or(0) as i32,
        Err(e) => {
            tracing::error!("Failed to get total sessions count: {}", e);
            0
        }
    };

    // Get zone distribution (mock data for now - would need heart rate time series)
    let zone_distribution = generate_mock_zone_distribution();

    // Get last sync time
    let last_sync = match sqlx::query!(
        r#"
        SELECT MAX(created_at) as last_sync
        FROM health_data 
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_one(&**pool)
    .await
    {
        Ok(row) => row.last_sync,
        Err(_) => None,
    };

    // Calculate weekly stats
    let weekly_stats = calculate_weekly_stats(&pool, user_id, week_ago).await;

    // Calculate monthly trends
    let monthly_trend = calculate_monthly_trend(&pool, user_id, month_ago).await;

    let activity_summary = ActivitySummaryResponse {
        recent_workouts,
        total_sessions,
        zone_distribution,
        last_sync,
        weekly_stats,
        monthly_trend,
    };

    tracing::info!("Successfully retrieved activity summary for user: {}", claims.username);
    HttpResponse::Ok().json(json!({
        "success": true,
        "data": activity_summary
    }))
}

fn generate_mock_zone_distribution() -> HashMap<String, f32> {
    let mut zones = HashMap::new();
    zones.insert("Zone 1 (Active Recovery)".to_string(), 28.5);
    zones.insert("Zone 2 (Aerobic Base)".to_string(), 45.2);
    zones.insert("Zone 3 (Aerobic)".to_string(), 22.8);
    zones.insert("Zone 4 (Threshold)".to_string(), 15.3);
    zones.insert("Zone 5 (VO2 Max)".to_string(), 8.7);
    zones
}

async fn calculate_weekly_stats(pool: &PgPool, user_id: Uuid, since: DateTime<Utc>) -> WeeklyStats {
    // Get total calories burned this week
    let total_calories = match sqlx::query!(
        r#"
        SELECT SUM(active_energy_burned) as total_calories
        FROM health_data 
        WHERE user_id = $1 
        AND created_at >= $2
        AND active_energy_burned IS NOT NULL
        "#,
        user_id,
        since
    )
    .fetch_one(pool)
    .await
    {
        Ok(row) => row.total_calories.unwrap_or(0.0),
        Err(_) => 0.0,
    };

    // Estimate exercise time (very simplified - 1 calorie = ~1 minute)
    let total_exercise_time = (total_calories / 5.0) as i32; // More realistic ratio

    // Count different session types (simplified based on calorie burn)
    let session_counts = match sqlx::query!(
        r#"
        SELECT 
            COUNT(CASE WHEN active_energy_burned > 400 THEN 1 END) as high_intensity,
            COUNT(CASE WHEN active_energy_burned BETWEEN 200 AND 400 THEN 1 END) as moderate_intensity
        FROM health_data 
        WHERE user_id = $1 
        AND created_at >= $2
        AND active_energy_burned > 200
        "#,
        user_id,
        since
    )
    .fetch_one(pool)
    .await
    {
        Ok(row) => (
            row.high_intensity.unwrap_or(0) as i32,
            row.moderate_intensity.unwrap_or(0) as i32
        ),
        Err(_) => (0, 0),
    };

    WeeklyStats {
        total_calories,
        total_exercise_time,
        strength_sessions: session_counts.0, // High intensity assumed as strength
        cardio_sessions: session_counts.1,   // Moderate intensity assumed as cardio
    }
}

async fn calculate_monthly_trend(pool: &PgPool, user_id: Uuid, since: DateTime<Utc>) -> MonthlyTrend {
    // Get stat changes from the last month
    // This would need to be implemented with a proper stat_changes table
    // For now, we'll estimate based on activity level
    
    let activity_count = match sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM health_data 
        WHERE user_id = $1 
        AND created_at >= $2
        AND active_energy_burned > 200
        "#,
        user_id,
        since
    )
    .fetch_one(pool)
    .await
    {
        Ok(row) => row.count.unwrap_or(0) as i32,
        Err(_) => 0,
    };

    // Estimate gains based on activity (very simplified)
    let stamina_gain = (activity_count * 2).min(50); // Max 50 points gain per month
    let strength_gain = activity_count.min(30); // Max 30 points gain per month
    
    MonthlyTrend {
        stamina_gain,
        strength_gain,
    }
}

// Additional endpoint for detailed zone analysis
#[tracing::instrument(
    name = "Get heart rate zone analysis",
    skip(_pool, claims),
    fields(username = %claims.username)
)]
pub async fn get_zone_analysis(
    _pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    let _user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::BadRequest().json(json!({
                "error": "Invalid user ID"
            }));
        }
    };

    // For now, return mock zone analysis data
    // In a real implementation, this would analyze heart rate time series data
    let zone_analysis = json!({
        "zones": {
            "zone_1": {
                "name": "Active Recovery",
                "range": "50-60% HRR",
                "time_spent": 28.5,
                "percentage": 31.2,
                "benefits": ["Recovery", "Fat burning", "Base building"]
            },
            "zone_2": {
                "name": "Aerobic Base",
                "range": "60-70% HRR", 
                "time_spent": 45.2,
                "percentage": 49.5,
                "benefits": ["Aerobic capacity", "Endurance", "Fat metabolism"]
            },
            "zone_3": {
                "name": "Aerobic",
                "range": "70-80% HRR",
                "time_spent": 22.8,
                "percentage": 25.0,
                "benefits": ["Improved efficiency", "Lactate threshold"]
            },
            "zone_4": {
                "name": "Threshold",
                "range": "80-90% HRR",
                "time_spent": 15.3,
                "percentage": 16.8,
                "benefits": ["VO2 max", "Speed", "Power"]
            },
            "zone_5": {
                "name": "VO2 Max",
                "range": "90-100% HRR",
                "time_spent": 8.7,
                "percentage": 9.5,
                "benefits": ["Maximum power", "Neuromuscular development"]
            }
        },
        "total_time": 120.5,
        "avg_heart_rate": 142,
        "max_heart_rate": 185,
        "user_max_hr_estimate": 190
    });

    tracing::info!("Successfully retrieved zone analysis for user: {}", claims.username);
    HttpResponse::Ok().json(json!({
        "success": true,
        "data": zone_analysis
    }))
}