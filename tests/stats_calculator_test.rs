use evolveme_backend::game::stats_calculator::WorkoutStatsCalculator;
use evolveme_backend::models::workout_data::{HeartRateData, WorkoutDataSyncRequest};
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
    
    let workout_data = WorkoutDataSyncRequest {
        workout_uuid: Uuid::new_v4().to_string(),
        device_id: "test".to_string(),
        timestamp: now,
        workout_start: workout_start,
        workout_end: workout_end,
        heart_rate: Some(heart_rate_data),
        calories_burned: Some(150),
        image_url: None,
        video_url: None,
        approval_token: None,
    };

    let workout_stats = WorkoutStatsCalculator::calculate_stat_changes(&test_app.db_pool, user_id, &workout_data).await;
    
    // Around 5 minutes * 2 points per minute ≈ 10 stamina points (9-10 due to rounding)
    assert!(workout_stats.as_ref().unwrap().changes.stamina_change >= 9 && workout_stats.as_ref().unwrap().changes.stamina_change <= 10);
    assert_eq!(workout_stats.as_ref().unwrap().changes.strength_change, 0); // Zone 1 gives no strength
    // Reasoning should contain zone info or heart rate stats if available
    if let Some(ref zones) = workout_stats.as_ref().unwrap().zone_breakdown {
        assert!(zones.iter().any(|z| z.zone.contains("Zone1")));
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

    let workout_data = WorkoutDataSyncRequest {
        workout_uuid: Uuid::new_v4().to_string(),
        device_id: "test".to_string(),
        timestamp: now,
        workout_start: workout_start,
        workout_end: workout_end,
        heart_rate: Some(heart_rate_data),
        calories_burned: Some(225),
        image_url: None,
        video_url: None,
        approval_token: None,
    };

    let workout_stats = WorkoutStatsCalculator::calculate_stat_changes(&test_app.db_pool, user_id, &workout_data).await;
    
    // Around 3 minutes * 5 stamina + 1 strength points per minute (14-15 stamina, 2-3 strength due to rounding)
    assert!(workout_stats.as_ref().unwrap().changes.stamina_change >= 14 && workout_stats.as_ref().unwrap().changes.stamina_change <= 15);
    assert!(workout_stats.as_ref().unwrap().changes.strength_change >= 2 && workout_stats.as_ref().unwrap().changes.strength_change <= 3);
    // Reasoning should contain zone info or heart rate stats if available
    if let Some(ref zones) = workout_stats.as_ref().unwrap().zone_breakdown {
        assert!(zones.iter().any(|z| z.zone.contains("Zone2")));
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
    
    let workout_data = WorkoutDataSyncRequest {
        workout_uuid: Uuid::new_v4().to_string(),
        device_id: "test".to_string(),
        timestamp: now,
        workout_start: workout_start,
        workout_end: workout_end,
        heart_rate: Some(heart_rate_data),
        calories_burned: Some(300),
        image_url: None,
        video_url: None,
        approval_token: None,
    };

    let workout_stats = WorkoutStatsCalculator::calculate_stat_changes(&test_app.db_pool, user_id, &workout_data).await;
    
    // Around 2 minutes * 2 stamina + 5 strength points per minute (3-4 stamina, 9-10 strength due to rounding)
    assert!(workout_stats.as_ref().unwrap().changes.stamina_change >= 3 && workout_stats.as_ref().unwrap().changes.stamina_change <= 4);
    assert!(workout_stats.as_ref().unwrap().changes.strength_change >= 9 && workout_stats.as_ref().unwrap().changes.strength_change <= 10);
    // Reasoning should contain zone info or heart rate stats if available
    if let Some(ref zones) = workout_stats.as_ref().unwrap().zone_breakdown {
        assert!(zones.iter().any(|z| z.zone.contains("Zone4")));
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
    
    let workout_data = WorkoutDataSyncRequest {
        workout_uuid: Uuid::new_v4().to_string(),
        device_id: "test".to_string(),
        timestamp: now,
        workout_start: workout_start,
        workout_end: workout_end,
        heart_rate: Some(heart_rate_data),
        calories_burned: Some(400),
        image_url: None,
        video_url: None,
        approval_token: None,
    };

    let workout_stats = WorkoutStatsCalculator::calculate_stat_changes(&test_app.db_pool, user_id, &workout_data).await;
    
    // Around 1.5 minutes * 1 stamina + 8 strength points per minute (1-2 stamina, 11-12 strength due to rounding)
    assert!(workout_stats.as_ref().unwrap().changes.stamina_change >= 1 && workout_stats.as_ref().unwrap().changes.stamina_change <= 2);
    assert!(workout_stats.as_ref().unwrap().changes.strength_change >= 11 && workout_stats.as_ref().unwrap().changes.strength_change <= 12);
    // Reasoning should contain zone info or heart rate stats if available
    if let Some(ref zones) = workout_stats.as_ref().unwrap().zone_breakdown {
        assert!(zones.iter().any(|z| z.zone.contains("Zone5")));
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
    
    let workout_data = WorkoutDataSyncRequest {
        workout_uuid: Uuid::new_v4().to_string(),
        device_id: "test".to_string(),
        timestamp: now,
        workout_start: workout_start,
        workout_end: workout_end,
        heart_rate: None,
        calories_burned: Some(200),
        image_url: None,
        video_url: None,
        approval_token: None,
    };

    let workout_stats = WorkoutStatsCalculator::calculate_stat_changes(&test_app.db_pool, user_id, &workout_data).await;
    assert_eq!(workout_stats.as_ref().unwrap().changes.stamina_change, 0);
    assert_eq!(workout_stats.as_ref().unwrap().changes.strength_change, 0);
    assert_eq!(workout_stats.as_ref().unwrap().zone_breakdown.is_none(), true);
}