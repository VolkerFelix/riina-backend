use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};
use sqlx::PgPool;
use uuid::Uuid;
use std::error::Error;
use crate::services::game_evaluation_service::GameEvaluationService;
use crate::services::manage_game_service::ManageGameService;

pub struct SchedulerService {
    scheduler: Arc<Mutex<JobScheduler>>,
    pool: PgPool,
    redis_client: Arc<redis::Client>,
    // Track active season jobs by season_id -> job_id
    active_jobs: Arc<Mutex<HashMap<Uuid, Uuid>>>,
}

impl SchedulerService {
    pub async fn new_with_redis(pool: PgPool, redis_client: Arc<redis::Client>) -> Result<Self, Box<dyn Error>> {
        let scheduler = JobScheduler::new().await?;
        
        Ok(Self {
            scheduler: Arc::new(Mutex::new(scheduler)),
            pool,
            redis_client,
            active_jobs: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        let scheduler = self.scheduler.lock().await;
        
        // For now, just start the scheduler without loading from DB
        // Seasons will be scheduled when created via the API
        scheduler.start().await?;
        
        tracing::info!("✅ Scheduler service started successfully (dynamic scheduling mode)");
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), Box<dyn Error>> {
        let mut scheduler = self.scheduler.lock().await;
        scheduler.shutdown().await?;
        
        tracing::info!("🛑 Scheduler service stopped");
        Ok(())
    }

    /// Schedule complete game management cycle for a new season
    /// Uses every-minute schedule to handle all game durations efficiently
    pub async fn schedule_season(&self, season_id: Uuid, season_name: String) -> Result<(), JobSchedulerError> {
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
                
                let manage_games = ManageGameService::new(pool.clone(), redis_client.clone());
                let evaluate_games = GameEvaluationService::new(pool, redis_client);
                
                // Step 1: Run complete game cycle (start due games, finish ended games)
                match manage_games.run_game_cycle().await {
                    Ok((pending_games, live_games, started_games, finished_games)) => {
                        tracing::info!("✅ Season '{}' game cycle: {} pending, {} live, {} started, {} finished", 
                            season_name, pending_games.len(), live_games.len(), started_games.len(), finished_games.len());
                        
                        // Step 2: Evaluate any finished games
                        let finished_games_clone = finished_games.clone();
                        match evaluate_games.evaluate_finished_live_games(finished_games).await {
                            Ok(result) => {
                                tracing::info!("✅ Game day completed. Calculated final scores for {} games", result.len());
                            }
                            Err(e) => {
                                let error_msg = e.to_string();
                                tracing::error!("❌ Game day evaluation failed for games: {:?} - {}", finished_games_clone, error_msg);
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        tracing::error!("❌ Season '{}' game cycle failed: {}", season_name, error_msg);
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
        pub async fn unschedule_season(&self, season_id: Uuid) -> Result<(), Box<dyn Error>> {
        let mut active_jobs = self.active_jobs.lock().await;
        
        if let Some(job_id) = active_jobs.remove(&season_id) {
            let scheduler = self.scheduler.lock().await;
            scheduler.remove(&job_id).await?;
            tracing::info!("✅ Removed scheduling for season {}", season_id);
        }
        
        Ok(())
    }
}