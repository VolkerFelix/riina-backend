use serde::{Serialize, Deserialize};
use serde_json::json;
use chrono::{Utc, Duration, DateTime};
use uuid::Uuid;
use reqwest::{Client, Error};

use crate::common::utils::{create_test_user_and_login, UserRegLoginResponse};
use riina_backend::models::workout_data::HeartRateData;

pub enum WorkoutIntensity {
    Hard,
    Intense,
    Moderate,
    Light,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkoutData {
    pub workout_uuid: String,
    pub workout_start: DateTime<Utc>,
    pub workout_end: DateTime<Utc>,
    pub calories_burned: i32,
    pub activity_name: Option<String>,
    pub heart_rate: Vec<serde_json::Value>,
    pub device_id: String,
    timestamp: DateTime<Utc>,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub approval_token: Option<String>,
}
impl WorkoutData {
    pub fn new(workout_type: WorkoutIntensity, workout_start: DateTime<Utc>, duration_minutes: i64) -> Self {
        let (heart_rate_data, calories_burned) = match workout_type {
            WorkoutIntensity::Hard => generate_hard_workout_data(workout_start, duration_minutes, None),
            WorkoutIntensity::Intense => generate_intense_workout_data(workout_start, duration_minutes),
            WorkoutIntensity::Moderate => generate_moderate_workout_data(workout_start, duration_minutes),
            WorkoutIntensity::Light => generate_light_workout_data(workout_start, duration_minutes),
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

    pub fn new_with_hr_freq(workout_type: WorkoutIntensity, workout_start: DateTime<Utc>, duration_minutes: i64, hr_freq_per_sec: Option<i32>) -> Self {
        let (heart_rate_data, calories_burned) = match workout_type {
            WorkoutIntensity::Hard => generate_hard_workout_data(workout_start, duration_minutes, hr_freq_per_sec),
            WorkoutIntensity::Intense => generate_intense_workout_data(workout_start, duration_minutes),
            WorkoutIntensity::Moderate => generate_moderate_workout_data(workout_start, duration_minutes),
            WorkoutIntensity::Light => generate_light_workout_data(workout_start, duration_minutes),
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
    
    pub fn new_with_offset_hours(workout_type: WorkoutIntensity, hours_ago: i64, duration_minutes: i64) -> Self {
        let workout_start = Utc::now() - Duration::hours(hours_ago);
        Self::new(workout_type, workout_start, duration_minutes)
    }

    pub fn get_heart_rate_data(&self) -> Vec<HeartRateData> {
        self.heart_rate.iter().map(|hr| HeartRateData {
            timestamp: hr["timestamp"].as_str().unwrap().parse().unwrap(),
            heart_rate: hr["heart_rate"].as_i64().unwrap() as i32,
        }).collect()
    }
}

fn generate_hard_workout_data(start_time: DateTime<Utc>, duration_min: i64, hr_freq_per_sec: Option<i32>) -> (Vec<serde_json::Value>, i32) {
    let mut heart_rate_data = Vec::new();
    let duration_sec = duration_min * 60;
    match hr_freq_per_sec {
        Some(hr_freq) => {
            // Reproduce Baeschor's data
            let start_time_off = start_time + Duration::milliseconds(796);
            // Example: hr_req_per_min = 120, then hr_req_per_sec = 120 / 60 = 2
            // So we need to generate 2 heart rate data points per second
            for i in 0..duration_sec {
                for j in 0..hr_freq {
                    heart_rate_data.push(json!({
                        "timestamp": start_time_off + Duration::seconds(i) + Duration::milliseconds((j * 500) as i64),
                        "heart_rate": 150 + (i % 40) // Very high intensity heart rate
                    }));
                }
            }
        }
        None => {
            heart_rate_data = (0..=(duration_min)).map(|i| json!({
                "timestamp": start_time + Duration::minutes(i),
                "heart_rate": 150 + (i % 40) as i32 // High intensity heart rate
            })).collect();
        }
    }
    println!("Heart rate samples: {:?}", heart_rate_data.len());
    (heart_rate_data, 1400)
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

pub async fn create_test_user_with_health_profile(app_address: &str) -> UserRegLoginResponse {
    let client = reqwest::Client::new();
    let user = create_test_user_and_login(app_address).await;
    
    // Create health profile for stats calculation
    let health_profile_data = json!({
        "age": 25,
        "gender": "male", 
        "resting_heart_rate": 60
    });
    
    let profile_response = client
        .put(&format!("{}/profile/health_profile", app_address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&health_profile_data)
        .send()
        .await
        .expect("Failed to create health profile");
    
    assert!(profile_response.status().is_success(), "Health profile creation should succeed");
    
    user
}

pub async fn create_health_profile_for_user(client: &Client, app_address: &str, user: &UserRegLoginResponse) -> Result<(), Error> {
    // Create health profile for stats calculation
    let health_profile_data = json!({
        "age": 25,
        "gender": "male", 
        "resting_heart_rate": 60
    });
    
    let profile_response = client
        .put(&format!("{}/profile/health_profile", app_address))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&health_profile_data)
        .send()
        .await?;
    
    assert!(profile_response.status().is_success(), "Health profile creation should succeed");

    Ok(())
}