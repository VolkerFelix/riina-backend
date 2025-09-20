use serde::{Serialize, Deserialize};
use serde_json::json;
use chrono::{Utc, Duration, DateTime};
use uuid::Uuid;

pub enum WorkoutType {
    Intense,
    Moderate,
    Light
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkoutData {
    pub workout_uuid: String,
    pub workout_start: DateTime<Utc>,
    pub workout_end: DateTime<Utc>,
    pub calories_burned: i32,
    pub activity_name: Option<String>,
    heart_rate: Vec<serde_json::Value>,
    pub device_id: String,
    timestamp: DateTime<Utc>,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub approval_token: Option<String>,
}
impl WorkoutData {
    pub fn new(workout_type: WorkoutType, workout_start: DateTime<Utc>, duration_minutes: i64) -> Self {
        let (heart_rate_data, calories_burned) = match workout_type {
            WorkoutType::Intense => generate_intense_workout_data(workout_start, duration_minutes),
            WorkoutType::Moderate => generate_moderate_workout_data(workout_start, duration_minutes),
            WorkoutType::Light => generate_light_workout_data(workout_start, duration_minutes),
        };
        
        let workout_uuid = Uuid::new_v4().to_string();
        let workout_end = workout_start + Duration::minutes(duration_minutes);
        Self {
            workout_uuid,
            workout_start,
            workout_end,
            calories_burned,
            activity_name: Some("Running".to_string()),
            heart_rate: heart_rate_data,
            device_id: format!("test-device-{}", &Uuid::new_v4().to_string()[..8]),
            timestamp: Utc::now(),
            image_url: None,
            video_url: None,
            approval_token: None,
        }
    }
    
    pub fn new_with_offset_hours(workout_type: WorkoutType, hours_ago: i64, duration_minutes: i64) -> Self {
        let workout_start = Utc::now() - Duration::hours(hours_ago);
        Self::new(workout_type, workout_start, duration_minutes)
    }
}


// Helper functions for generating workout data
fn generate_intense_workout_data(start_time: DateTime<Utc>, duration_minutes: i64) -> (Vec<serde_json::Value>, i32) {
    // Need duration_minutes + 1 points to get duration_minutes intervals
    let heart_rate_data = (0..=(duration_minutes)).map(|i| json!({
        "timestamp": start_time + Duration::minutes(i),
        "heart_rate": 150 + (i % 25) as i32 // High intensity heart rate
    })).collect();
    let calories = 450; // High intensity burns more calories
    (heart_rate_data, calories)
}

fn generate_moderate_workout_data(start_time: DateTime<Utc>, duration_minutes: i64) -> (Vec<serde_json::Value>, i32) {
    // Need duration_minutes + 1 points to get duration_minutes intervals
    let heart_rate_data = (0..=(duration_minutes)).map(|i| json!({
        "timestamp": start_time + Duration::minutes(i),
        "heart_rate": 110 + (i % 20) // Moderate intensity
    })).collect();
    let calories = 300; // Moderate intensity
    (heart_rate_data, calories)
}

fn generate_light_workout_data(start_time: DateTime<Utc>, duration_minutes: i64) -> (Vec<serde_json::Value>, i32) {
    // Need duration_minutes + 1 points to get duration_minutes intervals
    let heart_rate_data = (0..=(duration_minutes)).map(|i| json!({
        "timestamp": start_time + Duration::minutes(i),
        "heart_rate": 90 + (i % 15) // Light intensity
    })).collect();
    let calories = 180; // Light intensity
    (heart_rate_data, calories)
}

#[derive(Debug, Serialize)]
pub struct WorkoutSyncRequest {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub calories: i32,
    pub id: String,
}

/// Helper function to upload health data for a user
pub async fn upload_workout_data_for_user(
    client: &reqwest::Client,
    app_address: &str,
    token: &str,
    workout_data: &mut WorkoutData,
) -> Result<serde_json::Value, String> {
    let workout_sync_request = WorkoutSyncRequest {
        start: workout_data.workout_start,
        end: workout_data.workout_end,
        calories: workout_data.calories_burned,
        id: workout_data.workout_uuid.clone(),
    };

    let sync_response = crate::common::utils::make_authenticated_request(
        client,
        reqwest::Method::POST,
        &format!("{}/health/check_sync_status", app_address),
        token,
        Some(json!({
            "workouts": [workout_sync_request]
        })),
    ).await;
    if !sync_response.status().is_success() {
        let status = sync_response.status();
        let error_body = sync_response.text().await.map_err(|e| e.to_string())?;
        return Err(format!("Health data sync failed with status {}: {}", status, error_body));
    }
    let sync_response_data: serde_json::Value = sync_response.json().await.map_err(|e| e.to_string())?;
    
    // Check if we have the new approved_workouts format
    if let Some(approved_workouts) = sync_response_data["data"]["approved_workouts"].as_array() {
        // Find the approval for this workout
        for approval in approved_workouts {
            if approval["workout_id"].as_str() == Some(&workout_data.workout_uuid) {
                if let Some(token) = approval["approval_token"].as_str() {
                    workout_data.approval_token = Some(token.to_string());
                    break;
                }
            }
        }
    }
    let response = crate::common::utils::make_authenticated_request(
        client,
        reqwest::Method::POST,
        &format!("{}/health/upload_health", app_address),
        token,
        Some(json!(workout_data)),
    ).await;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.map_err(|e| e.to_string())?;
        return Err(format!("Health data upload failed with status {}: {}", status, error_body));
    }

    let response_data: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(response_data)
}