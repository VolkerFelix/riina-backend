use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{Local, Timelike, Datelike, NaiveDate};
use crate::services::game_evaluation_service::GameEvaluationService;
use crate::models::league::LeagueSeason;

pub struct SchedulerService {
    scheduler: Arc<Mutex<JobScheduler>>,
    pool: PgPool,
    redis_client: Option<Arc<redis::Client>>,
    // Track active season jobs by season_id -> job_id
    active_jobs: Arc<Mutex<HashMap<Uuid, Uuid>>>,
}

impl SchedulerService {
    pub async fn new(pool: PgPool) -> Result<Self, Box<dyn std::error::Error>> {
        let scheduler = JobScheduler::new().await?;
        
        Ok(Self {
            scheduler: Arc::new(Mutex::new(scheduler)),
            pool,
            redis_client: None,
            active_jobs: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn new_with_redis(pool: PgPool, redis_client: Option<Arc<redis::Client>>) -> Result<Self, Box<dyn std::error::Error>> {
        let scheduler = JobScheduler::new().await?;
        
        Ok(Self {
            scheduler: Arc::new(Mutex::new(scheduler)),
            pool,
            redis_client,
            active_jobs: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut scheduler = self.scheduler.lock().await;
        
        // For now, just start the scheduler without loading from DB
        // Seasons will be scheduled when created via the API
        scheduler.start().await?;
        
        tracing::info!("✅ Scheduler service started successfully (dynamic scheduling mode)");
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut scheduler = self.scheduler.lock().await;
        scheduler.shutdown().await?;
        
        tracing::info!("🛑 Scheduler service stopped");
        Ok(())
    }

    async fn schedule_game_evaluation(&self, scheduler: &mut JobScheduler) -> Result<(), Box<dyn std::error::Error>> {
        let pool = self.pool.clone();
        let redis_client = self.redis_client.clone();
        
        // Run game evaluation every hour to check for completed games
        let evaluation_job = Job::new_async("0 0 * * * *", move |_uuid, _l| {
            let pool = pool.clone();
            let redis_client = redis_client.clone();
            Box::pin(async move {
                tracing::info!("🎮 Running scheduled game evaluation");
                
                let evaluation_service = GameEvaluationService::new_with_redis(pool, redis_client);
                
                match evaluation_service.evaluate_and_update_todays_games().await {
                    Ok(result) => {
                        tracing::info!("✅ Game evaluation completed: {}", result);
                        
                        // Log any errors that occurred during evaluation
                        if !result.errors.is_empty() {
                            for error in &result.errors {
                                tracing::error!("Game evaluation error: {}", error);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("❌ Game evaluation failed: {}", e);
                    }
                }
            })
        })?;
        
        scheduler.add(evaluation_job).await?;
        tracing::info!("📅 Scheduled game evaluation job for Saturdays at 22:00 UTC");
        
        // Also schedule a more frequent check for development/testing (every hour)
        if cfg!(debug_assertions) {
            let pool = self.pool.clone();
            let redis_client = self.redis_client.clone();
            
            let hourly_job = Job::new_async("0 0 * * * *", move |_uuid, _l| {
                let pool = pool.clone();
                let redis_client = redis_client.clone();
                Box::pin(async move {
                    let now = Local::now();
                    
                    // Only run on Saturday at 22:00 local time
                    if now.weekday() == chrono::Weekday::Sat && now.hour() == 22 {
                        tracing::info!("🎮 Running hourly game evaluation check (dev mode)");
                        
                        let evaluation_service = GameEvaluationService::new_with_redis(pool, redis_client);
                        
                        match evaluation_service.evaluate_and_update_todays_games().await {
                            Ok(result) => {
                                tracing::info!("✅ Dev mode evaluation completed: {}", result);
                            }
                            Err(e) => {
                                tracing::error!("❌ Dev mode evaluation failed: {}", e);
                            }
                        }
                    }
                })
            })?;
            
            scheduler.add(hourly_job).await?;
            tracing::info!("📅 Scheduled hourly evaluation check for development");
        }
        
        Ok(())
    }

    // Manual trigger for testing or admin use
    pub async fn trigger_game_evaluation(&self) -> Result<String, Box<dyn std::error::Error>> {
        tracing::info!("🎮 Manually triggering game evaluation");
        
        let evaluation_service = GameEvaluationService::new_with_redis(self.pool.clone(), self.redis_client.clone());
        
        let result = evaluation_service.evaluate_and_update_todays_games().await?;
        
        Ok(format!(
            "Game evaluation completed: {} games evaluated, {} updated successfully",
            result.games_evaluated,
            result.games_updated
        ))
    }

    // Evaluate games for a specific date
    pub async fn trigger_game_evaluation_for_date(&self, date: chrono::NaiveDate) -> Result<String, Box<dyn std::error::Error>> {
        tracing::info!("🎮 Manually triggering game evaluation for date: {}", date);
        
        let evaluation_service = GameEvaluationService::new_with_redis(self.pool.clone(), self.redis_client.clone());
        
        let result = evaluation_service.evaluate_and_update_games_for_date(date).await?;
        
        Ok(format!(
            "Game evaluation completed for {}: {} games evaluated, {} updated successfully",
            date,
            result.games_evaluated,
            result.games_updated
        ))
    }

    /// Schedule evaluation job for a new season
    pub async fn schedule_season(&self, season_id: Uuid, season_name: String, cron_expr: String) -> Result<(), Box<dyn std::error::Error>> {
        let mut scheduler = self.scheduler.lock().await;
        
        let pool = self.pool.clone();
        let redis_client = self.redis_client.clone();
        
        // Clone season_name before moving into closure
        let season_name_for_logging = season_name.clone();
        
        let evaluation_job = Job::new_async(&cron_expr, move |_uuid, _l| {
            let pool = pool.clone();
            let redis_client = redis_client.clone();
            let season_name = season_name.clone();
            
            Box::pin(async move {
                tracing::info!("🎮 Running scheduled game evaluation for season '{}'", season_name);
                
                let evaluation_service = GameEvaluationService::new_with_redis(pool, redis_client);
                
                // Evaluate games for today's date
                let today = chrono::Utc::now().date_naive();
                match evaluation_service.evaluate_and_update_games_for_date(today).await {
                    Ok(result) => {
                        tracing::info!("✅ Season '{}' evaluation completed: {}", season_name, result);
                        
                        // Log any errors that occurred during evaluation
                        if !result.errors.is_empty() {
                            for error in &result.errors {
                                tracing::error!("Season '{}' evaluation error: {}", season_name, error);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("❌ Season '{}' evaluation failed: {}", season_name, e);
                    }
                }
            })
        })?;
        
        let job_id = evaluation_job.guid();
        scheduler.add(evaluation_job).await?;
        
        // Track the job
        let mut active_jobs = self.active_jobs.lock().await;
        active_jobs.insert(season_id, job_id);
        
        tracing::info!("✅ Scheduled evaluation for season '{}' with cron '{}'", season_name_for_logging, cron_expr);
        
        Ok(())
    }

    /// Remove scheduling for a season (when season is deleted)
    pub async fn unschedule_season(&self, season_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        let mut active_jobs = self.active_jobs.lock().await;
        
        if let Some(job_id) = active_jobs.remove(&season_id) {
            let mut scheduler = self.scheduler.lock().await;
            scheduler.remove(&job_id).await?;
            tracing::info!("✅ Removed scheduling for season {}", season_id);
        }
        
        Ok(())
    }
}