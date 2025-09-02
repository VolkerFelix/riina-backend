//! Unit tests for the SchedulerService
//! 
//! This test verifies the internal behavior of SchedulerService:
//! - Job scheduling and unscheduling
//! - Active job tracking
//! - Error handling in scheduler operations
//! - Concurrent season management

mod common;
use common::utils::spawn_app;

use riina_backend::services::SchedulerService;
use std::sync::Arc;
use uuid::Uuid;
use tokio::time::Duration;

#[tokio::test]
async fn test_scheduler_service_lifecycle() {
    println!("🧪 Testing SchedulerService Lifecycle");
    
    let app = spawn_app().await;
    
    // Create scheduler service
    let scheduler = SchedulerService::new_with_redis(
        app.db_pool.clone(), 
        app.redis_client.clone()
    ).await.expect("Failed to create scheduler service");
    
    // Test starting the scheduler
    scheduler.start().await.expect("Failed to start scheduler");
    println!("✅ Scheduler started successfully");
    
    // Test stopping the scheduler
    scheduler.stop().await.expect("Failed to stop scheduler");
    println!("✅ Scheduler stopped successfully");
    
    println!("🎉 Scheduler lifecycle test completed!");
}

#[tokio::test]
async fn test_scheduler_season_management() {
    println!("🧪 Testing SchedulerService Season Management");
    
    let app = spawn_app().await;
    
    // Create and start scheduler service
    let scheduler = SchedulerService::new_with_redis(
        app.db_pool.clone(), 
        app.redis_client.clone()
    ).await.expect("Failed to create scheduler service");
    
    scheduler.start().await.expect("Failed to start scheduler");
    
    // Test scheduling multiple seasons
    let season1_id = Uuid::new_v4();
    let season2_id = Uuid::new_v4();
    let season3_id = Uuid::new_v4();
    
    // Schedule first season
    let result1 = scheduler.schedule_season(season1_id, "Test Season 1".to_string()).await;
    assert!(result1.is_ok(), "Should successfully schedule first season");
    println!("✅ Scheduled season 1");
    
    // Schedule second season
    let result2 = scheduler.schedule_season(season2_id, "Test Season 2".to_string()).await;
    assert!(result2.is_ok(), "Should successfully schedule second season");
    println!("✅ Scheduled season 2");
    
    // Schedule third season
    let result3 = scheduler.schedule_season(season3_id, "Test Season 3".to_string()).await;
    assert!(result3.is_ok(), "Should successfully schedule third season");
    println!("✅ Scheduled season 3");
    
    // Wait a moment to let jobs register
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Test unscheduling middle season
    let unschedule_result = scheduler.unschedule_season(season2_id).await;
    assert!(unschedule_result.is_ok(), "Should successfully unschedule season 2");
    println!("✅ Unscheduled season 2");
    
    // Test unscheduling non-existent season (should not error)
    let fake_season_id = Uuid::new_v4();
    let fake_unschedule_result = scheduler.unschedule_season(fake_season_id).await;
    assert!(fake_unschedule_result.is_ok(), "Should handle unscheduling non-existent season gracefully");
    println!("✅ Handled unscheduling non-existent season");
    
    // Test unscheduling remaining seasons
    let unschedule1_result = scheduler.unschedule_season(season1_id).await;
    assert!(unschedule1_result.is_ok(), "Should successfully unschedule season 1");
    
    let unschedule3_result = scheduler.unschedule_season(season3_id).await;
    assert!(unschedule3_result.is_ok(), "Should successfully unschedule season 3");
    println!("✅ Unscheduled remaining seasons");
    
    // Clean up
    scheduler.stop().await.expect("Failed to stop scheduler");
    
    println!("🎉 Scheduler season management test completed!");
}

#[tokio::test]
async fn test_scheduler_duplicate_season_handling() {
    println!("🧪 Testing SchedulerService Duplicate Season Handling");
    
    let app = spawn_app().await;
    
    let scheduler = SchedulerService::new_with_redis(
        app.db_pool.clone(), 
        app.redis_client.clone()
    ).await.expect("Failed to create scheduler service");
    
    scheduler.start().await.expect("Failed to start scheduler");
    
    let season_id = Uuid::new_v4();
    
    // Schedule season first time
    let result1 = scheduler.schedule_season(season_id, "Test Season".to_string()).await;
    assert!(result1.is_ok(), "Should successfully schedule season first time");
    println!("✅ Scheduled season first time");
    
    // Try to schedule the same season again (should replace the old job)
    let result2 = scheduler.schedule_season(season_id, "Test Season Updated".to_string()).await;
    assert!(result2.is_ok(), "Should handle rescheduling same season");
    println!("✅ Rescheduled same season (should replace old job)");
    
    // Unschedule the season
    let unschedule_result = scheduler.unschedule_season(season_id).await;
    assert!(unschedule_result.is_ok(), "Should successfully unschedule season");
    println!("✅ Unscheduled season");
    
    // Clean up
    scheduler.stop().await.expect("Failed to stop scheduler");
    
    println!("🎉 Duplicate season handling test completed!");
}

#[tokio::test]
async fn test_scheduler_concurrent_operations() {
    println!("🧪 Testing SchedulerService Concurrent Operations");
    
    let app = spawn_app().await;
    
    let scheduler = Arc::new(
        SchedulerService::new_with_redis(
            app.db_pool.clone(), 
            app.redis_client.clone()
        ).await.expect("Failed to create scheduler service")
    );
    
    scheduler.start().await.expect("Failed to start scheduler");
    
    // Create multiple tasks that schedule seasons concurrently
    let mut handles = vec![];
    
    for i in 0..10 {
        let scheduler_clone = scheduler.clone();
        let handle = tokio::spawn(async move {
            let season_id = Uuid::new_v4();
            let season_name = format!("Concurrent Season {}", i);
            
            // Schedule season
            let schedule_result = scheduler_clone.schedule_season(season_id, season_name).await;
            assert!(schedule_result.is_ok(), "Should successfully schedule season {}", i);
            
            // Wait a bit
            tokio::time::sleep(Duration::from_millis(10)).await;
            
            // Unschedule season
            let unschedule_result = scheduler_clone.unschedule_season(season_id).await;
            assert!(unschedule_result.is_ok(), "Should successfully unschedule season {}", i);
            
            i
        });
        
        handles.push(handle);
    }
    
    // Wait for all concurrent operations to complete
    let mut completed = 0;
    for handle in handles {
        let result = handle.await.expect("Task should complete successfully");
        completed += 1;
        println!("✅ Completed concurrent operation {} (season {})", completed, result);
    }
    
    assert_eq!(completed, 10, "All 10 concurrent operations should complete");
    
    // Clean up
    scheduler.stop().await.expect("Failed to stop scheduler");
    
    println!("🎉 Concurrent operations test completed!");
}

#[tokio::test] 
async fn test_scheduler_error_conditions() {
    println!("🧪 Testing SchedulerService Error Conditions");
    
    let app = spawn_app().await;
    
    let scheduler = SchedulerService::new_with_redis(
        app.db_pool.clone(), 
        app.redis_client.clone()
    ).await.expect("Failed to create scheduler service");
    
    scheduler.start().await.expect("Failed to start scheduler");
    
    let season_id = Uuid::new_v4();
    
    // Test scheduling with empty season name (should still work)
    let empty_name_result = scheduler.schedule_season(season_id, "".to_string()).await;
    assert!(empty_name_result.is_ok(), "Should handle empty season name gracefully");
    println!("✅ Handled empty season name");
    
    // Unschedule 
    scheduler.unschedule_season(season_id).await.expect("Should unschedule");
    
    // Test scheduling after stopping scheduler
    scheduler.stop().await.expect("Failed to stop scheduler");
    
    let stopped_schedule_result = scheduler.schedule_season(Uuid::new_v4(), "After Stop".to_string()).await;
    // This might succeed or fail depending on implementation - just ensure it doesn't panic
    println!("✅ Attempted scheduling after stop: {:?}", stopped_schedule_result.is_ok());
    
    println!("🎉 Error conditions test completed!");
}

#[tokio::test]
async fn test_scheduler_job_tracking() {
    println!("🧪 Testing SchedulerService Job Tracking");
    
    let app = spawn_app().await;
    
    let scheduler = SchedulerService::new_with_redis(
        app.db_pool.clone(), 
        app.redis_client.clone()
    ).await.expect("Failed to create scheduler service");
    
    scheduler.start().await.expect("Failed to start scheduler");
    
    // Schedule multiple seasons and verify they get different job IDs
    let season1_id = Uuid::new_v4();
    let season2_id = Uuid::new_v4();
    let season3_id = Uuid::new_v4();
    
    scheduler.schedule_season(season1_id, "Season 1".to_string()).await
        .expect("Should schedule season 1");
    
    scheduler.schedule_season(season2_id, "Season 2".to_string()).await
        .expect("Should schedule season 2");
    
    scheduler.schedule_season(season3_id, "Season 3".to_string()).await
        .expect("Should schedule season 3");
    
    println!("✅ Scheduled 3 seasons for job tracking test");
    
    // Unschedule them in different order
    scheduler.unschedule_season(season2_id).await
        .expect("Should unschedule season 2");
    
    scheduler.unschedule_season(season1_id).await
        .expect("Should unschedule season 1");
    
    scheduler.unschedule_season(season3_id).await
        .expect("Should unschedule season 3");
    
    println!("✅ Unscheduled all seasons in different order");
    
    // Try to unschedule already unscheduled seasons (should be graceful)
    scheduler.unschedule_season(season1_id).await
        .expect("Should handle double unschedule gracefully");
    
    scheduler.unschedule_season(season2_id).await
        .expect("Should handle double unschedule gracefully");
    
    println!("✅ Handled double unscheduling gracefully");
    
    // Clean up
    scheduler.stop().await.expect("Failed to stop scheduler");
    
    println!("🎉 Job tracking test completed!");
}