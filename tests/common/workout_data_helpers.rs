use serde_json::json;
use chrono::{Utc, Duration};
use uuid::Uuid;

/// Health data generation helpers for different fitness levels
/// These functions create realistic heart rate data simulating different workout intensities

/// Beginner workout: 8 minutes, Zone 1-2 (110-135 bpm), lower calories
pub fn create_beginner_workout_data() -> serde_json::Value {
    let base_time = Utc::now();
    let mut heart_rate_readings = Vec::new();
    
    // Beginner: 8 minutes, Zone 1-2 workout (110-135 bpm)
    for i in 0..480 { // 480 seconds = 8 minutes
        let time_offset = Duration::seconds(i);
        let workout_progress = i as f64 / 480.0;
        
        let heart_rate = if workout_progress < 0.2 {
            // Warmup: 70-110 bpm
            (70.0 + 40.0 * workout_progress * 5.0) as i32
        } else if workout_progress < 0.8 {
            // Main workout: 110-135 bpm (Zone 1-2)
            (110.0 + 25.0 * (workout_progress - 0.2) / 0.6) as i32
        } else {
            // Cooldown: 135-90 bpm
            (135.0 - 45.0 * (workout_progress - 0.8) / 0.2) as i32
        };
        
        heart_rate_readings.push(json!({
            "timestamp": base_time + time_offset,
            "heart_rate": heart_rate
        }));
    }

    json!({
        "device_id": format!("device-{}", Uuid::new_v4()),
        "timestamp": base_time,
        "heart_rate": heart_rate_readings,
        "calories_burned": 180, // Lower calories for beginner
        "workout_start": base_time,
        "workout_end": base_time + Duration::seconds(480),
        "workout_uuid": &Uuid::new_v4().to_string()[..8]
    })
}

/// Intermediate workout: 15 minutes, Zone 2-3 (130-155 bpm), moderate calories
pub fn create_intermediate_workout_data() -> serde_json::Value {
    let base_time = Utc::now();
    let mut heart_rate_readings = Vec::new();
    
    // Intermediate: 15 minutes, Zone 2-3 workout (130-155 bpm)
    for i in 0..900 { // 900 seconds = 15 minutes
        let time_offset = Duration::seconds(i);
        let workout_progress = i as f64 / 900.0;
        
        let heart_rate = if workout_progress < 0.15 {
            // Warmup: 80-130 bpm
            (80.0 + 50.0 * workout_progress * 6.67) as i32
        } else if workout_progress < 0.85 {
            // Main workout: 130-155 bpm (Zone 2-3) with some variation
            let base_hr = 130.0 + 25.0 * (workout_progress - 0.15) / 0.7;
            (base_hr + 8.0 * (i as f64 * 0.05).sin()) as i32
        } else {
            // Cooldown: 155-95 bpm
            (155.0 - 60.0 * (workout_progress - 0.85) / 0.15) as i32
        };
        
        heart_rate_readings.push(json!({
            "timestamp": base_time + time_offset,
            "heart_rate": heart_rate
        }));
    }

    json!({
        "device_id": format!("device-{}", Uuid::new_v4()),
        "timestamp": base_time,
        "heart_rate": heart_rate_readings,
        "calories_burned": 320, // Moderate calories
        "workout_start": base_time,
        "workout_end": base_time + Duration::seconds(1500),
        "workout_uuid": &Uuid::new_v4().to_string()[..8]
    })
}

/// Advanced workout: 25 minutes, Zone 3-4 (150-175 bpm), higher calories with intervals
pub fn create_advanced_workout_data() -> serde_json::Value {
    let base_time = Utc::now();
    let mut heart_rate_readings = Vec::new();
    
    // Advanced: 25 minutes, Zone 3-4 workout (150-175 bpm)
    for i in 0..1500 { // 1500 seconds = 25 minutes
        let time_offset = Duration::seconds(i);
        let workout_progress = i as f64 / 1500.0;
        
        let heart_rate = if workout_progress < 0.1 {
            // Warmup: 85-150 bpm
            (85.0 + 65.0 * workout_progress * 10.0) as i32
        } else if workout_progress < 0.9 {
            // Main workout: 150-175 bpm (Zone 3-4) with intervals
            let base_hr = 150.0 + 25.0 * (workout_progress - 0.1) / 0.8;
            // Add interval pattern
            let interval_factor = if (i / 120) % 2 == 0 { 1.1 } else { 0.95 };
            (base_hr * interval_factor + 5.0 * (i as f64 * 0.02).sin()) as i32
        } else {
            // Cooldown: 175-100 bpm
            (175.0 - 75.0 * (workout_progress - 0.9) / 0.1) as i32
        };
        
        heart_rate_readings.push(json!({
            "timestamp": base_time + time_offset,
            "heart_rate": heart_rate
        }));
    }

    json!({
        "device_id": format!("device-{}", Uuid::new_v4()),
        "timestamp": base_time,
        "heart_rate": heart_rate_readings,
        "calories_burned": 520, // Higher calories for advanced
        "workout_start": base_time,
        "workout_end": base_time + Duration::seconds(2100),
        "workout_uuid": &Uuid::new_v4().to_string()[..8]
    })
}

/// Elite workout: 35 minutes, Zone 4-5 (165-185+ bpm), highest calories with intense intervals
pub fn create_elite_workout_data() -> serde_json::Value {
    let base_time = Utc::now();
    let mut heart_rate_readings = Vec::new();
    
    // Elite: 35 minutes, Zone 4-5 workout (165-185+ bpm)
    for i in 0..2100 { // 2100 seconds = 35 minutes
        let time_offset = Duration::seconds(i);
        let workout_progress = i as f64 / 2100.0;
        
        let heart_rate = if workout_progress < 0.08 {
            // Warmup: 90-165 bpm
            (90.0 + 75.0 * workout_progress * 12.5) as i32
        } else if workout_progress < 0.92 {
            // Main workout: 165-185+ bpm (Zone 4-5) with high intensity intervals
            let base_hr = 165.0 + 20.0 * (workout_progress - 0.08) / 0.84;
            // Elite interval training pattern
            let interval_boost = if (i / 180) % 3 == 0 { 15.0 } else { 0.0 };
            (base_hr + interval_boost + 8.0 * (i as f64 * 0.01).sin()) as i32
        } else {
            // Cooldown: 185-105 bpm
            (185.0 - 80.0 * (workout_progress - 0.92) / 0.08) as i32
        };
        
        heart_rate_readings.push(json!({
            "timestamp": base_time + time_offset,
            "heart_rate": heart_rate
        }));
    }

    json!({
        "device_id": format!("device-{}", Uuid::new_v4()),
        "timestamp": base_time,
        "heart_rate": heart_rate_readings,
        "calories_burned": 720, // Highest calories for elite
        "workout_start": base_time,
        "workout_end": base_time + Duration::seconds(2100),
        "workout_uuid": &Uuid::new_v4().to_string()[..8]
    })
}

/// Helper function to upload health data for a user
pub async fn upload_workout_data_for_user(
    client: &reqwest::Client,
    app_address: &str,
    token: &str,
    health_data: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let response = crate::common::utils::make_authenticated_request(
        client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", app_address),
        token,
        Some(health_data),
    ).await;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.map_err(|e| e.to_string())?;
        return Err(format!("Health data upload failed with status {}: {}", status, error_body));
    }

    let response_data: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(response_data)
}