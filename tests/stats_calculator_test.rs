use riina_backend::game::stats_calculator::WorkoutStatsCalculator;
use riina_backend::models::workout_data::{HeartRateData, WorkoutDataUploadRequest};
use riina_backend::db::health_data::get_user_health_profile_details;
use chrono::{Duration, Utc};
use uuid::Uuid;

mod common;
use common::utils::spawn_app;

#[tokio::test]
async fn test_zone_1_active_recovery() {
    let test_app = spawn_app().await;
    
    // Create a test user with health profile
    let user_id = Uuid::new_v4();
    let username = format!("testuser_{}", &user_id.to_string()[..4]);
    let email = format!("test_{}@test.com", &user_id.to_string()[..4]);
    
    // Insert user and health profile for testing
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, created_at, updated_at) VALUES ($1, $2, $3, $4, NOW(), NOW())"
    )
    .bind(user_id)
    .bind(username)
    .bind(email)
    .bind("dummy_hash")
    .execute(&test_app.db_pool)
    .await
    .unwrap();
    
    sqlx::query(
        "INSERT INTO user_health_profiles (user_id, age, gender, resting_heart_rate) VALUES ($1, $2, $3, $4)"
    )
    .bind(user_id)
    .bind(25)
    .bind("male")
    .bind(60)
    .execute(&test_app.db_pool)
    .await
    .unwrap();

    // Create heart rate data for Zone 1 (around 50-60% HRR)
    let mut heart_rate_data = Vec::new();
    let now = Utc::now();
    let workout_start = now - Duration::minutes(30);
    let workout_end = now;
    let base_time = workout_start;
    
    // Generate 5 minutes of Zone 1 heart rate data
    for i in 0..300 { // 300 seconds = 5 minutes
        heart_rate_data.push(HeartRateData {
            timestamp: base_time + Duration::seconds(i),
            heart_rate: 130, // Zone 1 for 25-year-old male (resting 60, max ~190)
        });
    }
    
    let workout_data = WorkoutDataUploadRequest {
        workout_uuid: Uuid::new_v4().to_string(),
        device_id: "test".to_string(),
        timestamp: now,
        workout_start: workout_start,
        workout_end: workout_end,
        heart_rate: Some(heart_rate_data),
        calories_burned: Some(150),
        activity_name: None,
        image_url: None,
        video_url: None,
        approval_token: None,
    };

    let user_health_profile = get_user_health_profile_details(&test_app.db_pool, user_id).await.unwrap();
    let heart_rate_data = workout_data.heart_rate.clone().unwrap_or_default();

    let workout_stats = WorkoutStatsCalculator::with_hr_zone_based().calculate_stat_changes(user_health_profile, heart_rate_data).await;
    
    assert!(workout_stats.as_ref().unwrap().changes.stamina_change >= 0.0 && workout_stats.as_ref().unwrap().changes.stamina_change <= 10.0);
    if let Some(ref zones) = workout_stats.as_ref().unwrap().zone_breakdown {
        assert!(zones.iter().any(|z| z.zone.contains("Zone1")) || zones.iter().any(|z| z.zone.contains("Easy")));
    }
}

#[tokio::test]
async fn test_zone_2_aerobic_base() {
    let test_app = spawn_app().await;
    
    // Create a test user with health profile
    let user_id = Uuid::new_v4();
    let username = format!("testuser2_{}", &user_id.to_string()[..4]);
    let email = format!("test2_{}@test.com", &user_id.to_string()[..4]);
    
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, created_at, updated_at) VALUES ($1, $2, $3, $4, NOW(), NOW())"
    )
    .bind(user_id)
    .bind(username)
    .bind(email)
    .bind("dummy_hash")
    .execute(&test_app.db_pool)
    .await
    .unwrap();
    
    sqlx::query(
        "INSERT INTO user_health_profiles (user_id, age, gender, resting_heart_rate) VALUES ($1, $2, $3, $4)"
    )
    .bind(user_id)
    .bind(25)
    .bind("male")
    .bind(60)
    .execute(&test_app.db_pool)
    .await
    .unwrap();

    // Create heart rate data for Zone 2 (around 60-70% HRR)
    let mut heart_rate_data = Vec::new();
    let now = Utc::now();
    let workout_start = now - Duration::minutes(30);
    let workout_end = now;
    let base_time = workout_start;
    
    // Generate 3 minutes of Zone 2 heart rate data
    for i in 0..180 { // 180 seconds = 3 minutes
        heart_rate_data.push(HeartRateData {
            timestamp: base_time + Duration::seconds(i),
            heart_rate: 145, // Zone 2 for 25-year-old male (60-70% HRR)
        });
    }

    let workout_data = WorkoutDataUploadRequest {
        workout_uuid: Uuid::new_v4().to_string(),
        device_id: "test".to_string(),
        timestamp: now,
        workout_start: workout_start,
        workout_end: workout_end,
        heart_rate: Some(heart_rate_data),
        calories_burned: Some(225),
        activity_name: None,
        image_url: None,
        video_url: None,
        approval_token: None,
    };

    let user_health_profile = get_user_health_profile_details(&test_app.db_pool, user_id).await.unwrap();
    let heart_rate_data = workout_data.heart_rate.clone().unwrap_or_default();

    let workout_stats = WorkoutStatsCalculator::with_hr_zone_based().calculate_stat_changes(user_health_profile, heart_rate_data).await;
    
    assert!(workout_stats.as_ref().unwrap().changes.stamina_change >= 0.0 && workout_stats.as_ref().unwrap().changes.stamina_change <= 15.0);
    // Reasoning should contain zone info or heart rate stats if available
    if let Some(ref zones) = workout_stats.as_ref().unwrap().zone_breakdown {
        assert!(zones.iter().any(|z| z.zone.contains("Zone2")) || zones.iter().any(|z| z.zone.contains("Easy")));
    }
}

#[tokio::test]
async fn test_zone_4_lactate_threshold() {
    let test_app = spawn_app().await;
    
    // Create a test user with health profile
    let user_id = Uuid::new_v4();
    let username = format!("testuser4_{}", &user_id.to_string()[..4]);
    let email = format!("test4_{}@test.com", &user_id.to_string()[..4]);
    
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, created_at, updated_at) VALUES ($1, $2, $3, $4, NOW(), NOW())"
    )
    .bind(user_id)
    .bind(username)
    .bind(email)
    .bind("dummy_hash")
    .execute(&test_app.db_pool)
    .await
    .unwrap();
    
    sqlx::query(
        "INSERT INTO user_health_profiles (user_id, age, gender, resting_heart_rate) VALUES ($1, $2, $3, $4)"
    )
    .bind(user_id)
    .bind(25)
    .bind("male")
    .bind(60)
    .execute(&test_app.db_pool)
    .await
    .unwrap();

    // Create heart rate data for Zone 4 (around 80-90% HRR)
    let mut heart_rate_data = Vec::new();
    let now = Utc::now();
    let workout_start = now - Duration::minutes(30);
    let workout_end = now;
    let base_time = workout_start;
    
    // Generate 2 minutes of Zone 4 heart rate data
    for i in 0..120 { // 120 seconds = 2 minutes
        heart_rate_data.push(HeartRateData {
            timestamp: base_time + Duration::seconds(i),
            heart_rate: 170, // Zone 4 for 25-year-old male (80-90% HRR, 164-177 bpm)
        });
    }
    
    let workout_data = WorkoutDataUploadRequest {
        workout_uuid: Uuid::new_v4().to_string(),
        device_id: "test".to_string(),
        timestamp: now,
        workout_start: workout_start,
        workout_end: workout_end,
        heart_rate: Some(heart_rate_data),
        calories_burned: Some(300),
        activity_name: None,
        image_url: None,
        video_url: None,
        approval_token: None,
    };

    let user_health_profile = get_user_health_profile_details(&test_app.db_pool, user_id).await.unwrap();
    let heart_rate_data = workout_data.heart_rate.clone().unwrap_or_default();

    let workout_stats = WorkoutStatsCalculator::with_hr_zone_based().calculate_stat_changes(user_health_profile, heart_rate_data).await;
    
    assert!(workout_stats.as_ref().unwrap().changes.stamina_change >= 0.0 && workout_stats.as_ref().unwrap().changes.stamina_change <= 40.0);
    // Reasoning should contain zone info or heart rate stats if available
    if let Some(ref zones) = workout_stats.as_ref().unwrap().zone_breakdown {
        assert!(zones.iter().any(|z| z.zone.contains("Zone4")) || zones.iter().any(|z| z.zone.contains("Hard")));
    }
}

#[tokio::test]
async fn test_zone_5_neuromuscular_power() {
    let test_app = spawn_app().await;
    
    // Create a test user with health profile
    let user_id = Uuid::new_v4();
    let username = format!("testuser5_{}", &user_id.to_string()[..4]);
    let email = format!("test5_{}@test.com", &user_id.to_string()[..4]);
    
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, created_at, updated_at) VALUES ($1, $2, $3, $4, NOW(), NOW())"
    )
    .bind(user_id)
    .bind(username)
    .bind(email)
    .bind("dummy_hash")
    .execute(&test_app.db_pool)
    .await
    .unwrap();
    
    sqlx::query(
        "INSERT INTO user_health_profiles (user_id, age, gender, resting_heart_rate) VALUES ($1, $2, $3, $4)"
    )
    .bind(user_id)
    .bind(25)
    .bind("male")
    .bind(60)
    .execute(&test_app.db_pool)
    .await
    .unwrap();

    // Create heart rate data for Zone 5 (90%+ HRR)
    let mut heart_rate_data = Vec::new();
    let now = Utc::now();
    let workout_start = now - Duration::minutes(30);
    let workout_end = now;
    let base_time = workout_start;
    
    // Generate 1.5 minutes of Zone 5 heart rate data
    for i in 0..90 { // 90 seconds = 1.5 minutes
        heart_rate_data.push(HeartRateData {
            timestamp: base_time + Duration::seconds(i),
            heart_rate: 180, // Zone 5 for 25-year-old male (90%+ HRR, need >177 bpm)
        });
    }
    
    let workout_data = WorkoutDataUploadRequest {
        workout_uuid: Uuid::new_v4().to_string(),
        device_id: "test".to_string(),
        timestamp: now,
        workout_start: workout_start,
        workout_end: workout_end,
        heart_rate: Some(heart_rate_data),
        calories_burned: Some(400),
        activity_name: None,
        image_url: None,
        video_url: None,
        approval_token: None,
    };

    let user_health_profile = get_user_health_profile_details(&test_app.db_pool, user_id).await.unwrap();
    let heart_rate_data = workout_data.heart_rate.clone().unwrap_or_default();

    let workout_stats = WorkoutStatsCalculator::with_hr_zone_based().calculate_stat_changes(user_health_profile, heart_rate_data).await;
    
    assert!(workout_stats.as_ref().unwrap().changes.stamina_change >= 0.0 && workout_stats.as_ref().unwrap().changes.stamina_change <= 100.0);
    // Reasoning should contain zone info or heart rate stats if available
    if let Some(ref zones) = workout_stats.as_ref().unwrap().zone_breakdown {
        assert!(zones.iter().any(|z| z.zone.contains("Zone5")) || zones.iter().any(|z| z.zone.contains("Hard")));
    }
}

#[tokio::test]
async fn test_no_heart_rate_no_gains() {
    let test_app = spawn_app().await;
    
    // Create a test user with health profile
    let user_id = Uuid::new_v4();
    let username = format!("testuser_no_hr_{}", &user_id.to_string()[..4]);
    let email = format!("test_no_hr_{}@test.com", &user_id.to_string()[..4]);
    
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, created_at, updated_at) VALUES ($1, $2, $3, $4, NOW(), NOW())"
    )
    .bind(user_id)
    .bind(username)
    .bind(email)
    .bind("dummy_hash")
    .execute(&test_app.db_pool)
    .await
    .unwrap();
    
    sqlx::query(
        "INSERT INTO user_health_profiles (user_id, age, gender, resting_heart_rate) VALUES ($1, $2, $3, $4)"
    )
    .bind(user_id)
    .bind(25)
    .bind("male")
    .bind(60)
    .execute(&test_app.db_pool)
    .await
    .unwrap();

    let now = Utc::now();
    let workout_start = now - Duration::minutes(30);
    let workout_end = now;
    
    let workout_data = WorkoutDataUploadRequest {
        workout_uuid: Uuid::new_v4().to_string(),
        device_id: "test".to_string(),
        timestamp: now,
        workout_start: workout_start,
        workout_end: workout_end,
        heart_rate: None,
        calories_burned: Some(200),
        activity_name: None,
        image_url: None,
        video_url: None,
        approval_token: None,
    };

    let user_health_profile = get_user_health_profile_details(&test_app.db_pool, user_id).await.unwrap();
    let heart_rate_data = workout_data.heart_rate.clone().unwrap_or(Vec::new());

    let workout_stats = WorkoutStatsCalculator::with_universal_hr_based().calculate_stat_changes(user_health_profile, heart_rate_data).await;
    assert_eq!(workout_stats.as_ref().unwrap().changes.stamina_change, 0.0);
    assert_eq!(workout_stats.as_ref().unwrap().changes.strength_change, 0.0);
    assert_eq!(workout_stats.as_ref().unwrap().zone_breakdown.is_none(), true);
}