use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};
use sqlx::PgPool;
use uuid::Uuid;
use crate::services::game_evaluation_service::GameEvaluationService;
use crate::services::week_game_service::WeekGameService;

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
        let scheduler = self.scheduler.lock().await;
        
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


    // Manual trigger for testing or admin use
    pub async fn trigger_game_cycle(&self) -> Result<String, Box<dyn std::error::Error>> {
        tracing::info!("🎮 Manually triggering complete game management cycle");
        
        let week_game_service = WeekGameService::new(self.pool.clone());
        let evaluation_service = GameEvaluationService::new_with_redis(self.pool.clone(), self.redis_client.clone());
        
        // Run the complete cycle: start due games, finish ended games
        let (started_games, finished_games) = week_game_service.run_game_cycle().await?;
        
        // Then evaluate any finished games
        let result = evaluation_service.evaluate_and_update_todays_games().await?;
        
        Ok(format!(
            "Game cycle completed: {} games started, {} games finished, {} games evaluated, {} updated successfully",
            started_games.len(),
            finished_games.len(),
            result.games_evaluated,
            result.games_updated
        ))
    }

    // Evaluate games for a specific date
    pub async fn trigger_game_evaluation_for_date(&self, date: chrono::NaiveDate) -> Result<String, Box<dyn std::error::Error>> {
        tracing::info!("🎮 Manually triggering game evaluation for date: {}", date);
        
        let evaluation_service = GameEvaluationService::new_with_redis(self.pool.clone(), self.redis_client.clone());
        
        let result = evaluation_service.evaluate_and_update_games().await?;
        
        Ok(format!(
            "Game evaluation completed for {}: {} games evaluated, {} updated successfully",
            date,
            result.games_evaluated,
            result.games_updated
        ))
    }

    /// Schedule complete game management cycle for a new season
    /// Uses every-minute schedule to handle all game durations efficiently
    pub async fn schedule_season(&self, season_id: Uuid, season_name: String) -> Result<(), Box<dyn std::error::Error>> {
        let cron_expr = "0 * * * * *".to_string(); // Every minute
        
        let scheduler = self.scheduler.lock().await;
        
        let pool = self.pool.clone();
        let redis_client = self.redis_client.clone();
        
        // Clone season_name before moving into closure
        let season_name_for_logging = season_name.clone();
        
        let game_cycle_job = Job::new_async(&cron_expr, move |_uuid, _l| {
            let pool = pool.clone();
            let redis_client = redis_client.clone();
            let season_name = season_name.clone();
            
            Box::pin(async move {
                tracing::info!("🎮 Running scheduled game management cycle for season '{}'", season_name);
                
                let week_game_service = WeekGameService::new(pool.clone());
                let evaluation_service = GameEvaluationService::new_with_redis(pool, redis_client);
                
                // Step 1: Run complete game cycle (start due games, finish ended games)
                match week_game_service.run_game_cycle().await {
                    Ok((started_games, finished_games)) => {
                        tracing::info!("✅ Season '{}' game cycle: {} started, {} finished", 
                            season_name, started_games.len(), finished_games.len());
                        
                        // Step 2: Evaluate any finished games
                        match evaluation_service.evaluate_and_update_games().await {
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
                    }
                    Err(e) => {
                        tracing::error!("❌ Season '{}' game cycle failed: {}", season_name, e);
                    }
                }
            })
        })?;
        
        let job_id = game_cycle_job.guid();
        scheduler.add(game_cycle_job).await?;
        
        // Track the job
        let mut active_jobs = self.active_jobs.lock().await;
        active_jobs.insert(season_id, job_id);
        
        tracing::info!("✅ Scheduled complete game management cycle for season '{}' (every minute)", season_name_for_logging);
        
        Ok(())
    }

    /// Remove scheduling for a season (when season is deleted)
    pub async fn unschedule_season(&self, season_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        let mut active_jobs = self.active_jobs.lock().await;
        
        if let Some(job_id) = active_jobs.remove(&season_id) {
            let scheduler = self.scheduler.lock().await;
            scheduler.remove(&job_id).await?;
            tracing::info!("✅ Removed scheduling for season {}", season_id);
        }
        
        Ok(())
    }
}