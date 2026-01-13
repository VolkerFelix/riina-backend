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

        // Schedule poll expiration job
        let poll_job = self.create_poll_expiration_job()?;
        scheduler.add(poll_job).await?;

        scheduler.start().await?;

        tracing::info!("✅ [SCHEDULER] Service started successfully");

        // Release the lock before scheduling seasons
        drop(scheduler);

        // Load and schedule all active seasons from the database
        tracing::info!("🔍 [SCHEDULER] Loading active seasons from database...");
        match self.load_active_seasons().await {
            Ok(count) => {
                tracing::info!("✅ [SCHEDULER] Loaded and scheduled {} active seasons", count);
            }
            Err(e) => {
                tracing::error!("❌ [SCHEDULER] Failed to load active seasons: {}", e);
                // Don't fail startup if season loading fails
            }
        }

        Ok(())
    }

    /// Load all active seasons from the database and schedule them
    async fn load_active_seasons(&self) -> Result<usize, Box<dyn Error>> {
        // Query for all active seasons that have auto evaluation enabled
        let active_seasons = sqlx::query!(
            r#"
            SELECT id, name, game_duration_seconds, auto_evaluation_enabled
            FROM league_seasons
            WHERE auto_evaluation_enabled = true
            AND start_date <= NOW()
            AND end_date >= NOW()
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        tracing::info!("🔍 [SCHEDULER] Found {} active seasons with auto-evaluation enabled", active_seasons.len());

        for season in &active_seasons {
            tracing::info!("📅 [SCHEDULER] Scheduling season '{}' (id: {}, duration: {}s)",
                season.name, season.id, season.game_duration_seconds);

            // Calculate cron expression based on game duration (default to every minute)
            let cron_expr = "0 * * * * *"; // Run every minute

            match self.schedule_season_with_frequency(season.id, season.name.clone(), cron_expr).await {
                Ok(_) => {
                    tracing::info!("✅ [SCHEDULER] Scheduled season '{}'", season.name);
                }
                Err(e) => {
                    tracing::error!("❌ [SCHEDULER] Failed to schedule season '{}': {}", season.name, e);
                }
            }
        }

        Ok(active_seasons.len())
    }

    /// Create poll expiration job that runs every 5 minutes
    fn create_poll_expiration_job(&self) -> Result<Job, JobSchedulerError> {
        let pool = self.pool.clone();
        let redis_client = self.redis_client.clone();

        Job::new_async("0 */5 * * * *", move |_uuid, _l| {
            let pool = pool.clone();
            let redis_client = redis_client.clone();

            Box::pin(async move {
                tracing::info!("🗳️ [SCHEDULER] Running scheduled poll expiration check");

                // Find all active polls that have expired
                let expired_polls = match sqlx::query!(
                    r#"
                    SELECT id
                    FROM team_polls
                    WHERE status = 'active' AND expires_at < NOW()
                    "#
                )
                .fetch_all(&pool)
                .await
                {
                    Ok(polls) => polls,
                    Err(e) => {
                        tracing::error!("❌ [SCHEDULER] Failed to fetch expired polls: {}", e);
                        return;
                    }
                };

                if expired_polls.is_empty() {
                    tracing::debug!("[SCHEDULER] No expired polls found");
                    return;
                }

                tracing::info!("[SCHEDULER] Found {} expired polls to process", expired_polls.len());

                for poll in expired_polls {
                    if let Err(e) = Self::process_expired_poll(&pool, &redis_client, poll.id).await {
                        tracing::error!("❌ Failed to process expired poll {}: {}", poll.id, e);
                    }
                }
            })
        })
    }

    /// Process an expired poll - just mark it as expired
    async fn process_expired_poll(
        pool: &PgPool,
        _redis_client: &Arc<redis::Client>,
        poll_id: uuid::Uuid
    ) -> Result<(), Box<dyn Error>> {
        // Simply mark the poll as expired
        sqlx::query!(
            "UPDATE team_polls SET status = 'expired', executed_at = NOW() WHERE id = $1",
            poll_id
        )
        .execute(pool)
        .await?;

        tracing::info!("✅ Poll {} marked as expired", poll_id);

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), Box<dyn Error>> {
        let mut scheduler = self.scheduler.lock().await;
        scheduler.shutdown().await?;
        
        tracing::info!("🛑 [SCHEDULER] Service stopped");
        Ok(())
    }

    /// Schedule complete game management cycle for a new season
    /// Uses every-minute schedule to handle all game durations efficiently
    pub async fn schedule_season(&self, season_id: Uuid, season_name: String) -> Result<(), JobSchedulerError> {
        self.schedule_season_with_frequency(season_id, season_name, "0 * * * * *").await
    }

    /// Schedule complete game management cycle for a new season with custom frequency
    pub async fn schedule_season_with_frequency(&self, season_id: Uuid, season_name: String, cron_expr: &str) -> Result<(), JobSchedulerError> {
        let cron_expr = cron_expr.to_string();
        
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
                let now = chrono::Utc::now();
                tracing::info!("🎮 [SCHEDULER] Running scheduled game management cycle for season '{}' at {}", season_name, now.to_rfc3339());
                tracing::info!("🔍 [SCHEDULER] Checking for games to start and finish for season '{}'", season_name);

                let manage_games = ManageGameService::new(pool.clone());
                let evaluate_games = GameEvaluationService::new(pool, redis_client);

                // Step 1: Run complete game cycle (start due games, finish ended games)
                tracing::info!("⏰ [SCHEDULER] Step 1: Running game cycle (checking scheduled and in-progress games)");
                match manage_games.run_game_cycle().await {
                    Ok((games_ready_to_start, live_games, started_games, finished_games)) => {
                        tracing::info!("✅ [SCHEDULER] Season '{}' game cycle results:", season_name);
                        tracing::info!("   📋 Games ready to start (scheduled, start time reached): {:?}", games_ready_to_start);
                        tracing::info!("   🎮 Games currently live (in_progress): {:?}", live_games);
                        tracing::info!("   ▶️  Games just started: {:?}", started_games);
                        tracing::info!("   🏁 Games just finished: {:?}", finished_games);

                        if !finished_games.is_empty() {
                            // Step 2: Evaluate any finished games
                            tracing::info!("⏰ [SCHEDULER] Step 2: Evaluating {} finished games", finished_games.len());
                            match evaluate_games.evaluate_finished_live_games(&finished_games).await {
                                Ok(result) => {
                                    tracing::info!("✅ [SCHEDULER] Game evaluation completed. Calculated final scores for {} games", result.len());
                                }
                                Err(e) => {
                                    let error_msg = e.to_string();
                                    tracing::error!("❌ [SCHEDULER] Game evaluation failed for games: {:?} - {}", finished_games, error_msg);
                                }
                            }
                        } else {
                            tracing::info!("ℹ️  [SCHEDULER] No finished games to evaluate");
                        }

                        if started_games.is_empty() && finished_games.is_empty() {
                            tracing::info!("ℹ️  [SCHEDULER] No state changes this cycle (no games started or finished)");
                        }
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        tracing::error!("❌ [SCHEDULER] Season '{}' game cycle failed: {}", season_name, error_msg);
                    }
                }

                tracing::info!("🏁 [SCHEDULER] Completed cycle for season '{}' at {}", season_name, chrono::Utc::now().to_rfc3339());
            })
        })?;
        
        let job_id = game_cycle_job.guid();
        scheduler.add(game_cycle_job).await?;
        
        // Track the job
        let mut active_jobs = self.active_jobs.lock().await;
        active_jobs.insert(season_id, job_id);
        
        let now = chrono::Utc::now();
        tracing::info!("✅ [SCHEDULER] Scheduled complete game management cycle for season '{}' (job_id: {})",
            season_name_for_logging, job_id);
        tracing::info!("   📅 Cron expression: {} (runs every minute)", cron_expr);
        tracing::info!("   ⏰ Next run: within 1 minute from {}", now.to_rfc3339());
        
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

    /// Public test helper to process a specific expired poll
    /// Only available in test and debug builds
    #[cfg(any(test, debug_assertions))]
    pub async fn process_expired_poll_test(
        pool: &PgPool,
        redis_client: &Arc<redis::Client>,
        poll_id: uuid::Uuid
    ) -> Result<(), Box<dyn Error>> {
        Self::process_expired_poll(pool, redis_client, poll_id).await
    }
}