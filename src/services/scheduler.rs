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
        let mut scheduler = self.scheduler.lock().await;

        // Schedule poll expiration job
        let poll_job = self.create_poll_expiration_job()?;
        scheduler.add(poll_job).await?;

        // For now, just start the scheduler without loading from DB
        // Seasons will be scheduled when created via the API
        scheduler.start().await?;

        tracing::info!("✅ Scheduler service started successfully (dynamic scheduling mode)");
        Ok(())
    }

    /// Create poll expiration job that runs every 5 minutes
    fn create_poll_expiration_job(&self) -> Result<Job, JobSchedulerError> {
        let pool = self.pool.clone();

        Job::new_async("0 */5 * * * *", move |_uuid, _l| {
            let pool = pool.clone();

            Box::pin(async move {
                tracing::info!("🗳️ Running scheduled poll expiration check");

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
                        tracing::error!("❌ Failed to fetch expired polls: {}", e);
                        return;
                    }
                };

                if expired_polls.is_empty() {
                    tracing::debug!("No expired polls found");
                    return;
                }

                tracing::info!("Found {} expired polls to process", expired_polls.len());

                for poll in expired_polls {
                    if let Err(e) = Self::process_expired_poll(&pool, poll.id).await {
                        tracing::error!("❌ Failed to process expired poll {}: {}", poll.id, e);
                    }
                }
            })
        })
    }

    /// Process an expired poll
    async fn process_expired_poll(pool: &PgPool, poll_id: uuid::Uuid) -> Result<(), Box<dyn Error>> {
        use crate::models::team::PollResult;

        // Get poll information with vote counts
        let poll_data = sqlx::query!(
            r#"
            SELECT
                tp.id, tp.team_id, tp.target_user_id,
                t.team_name,
                target_user.username as target_username
            FROM team_polls tp
            JOIN teams t ON tp.team_id = t.id
            JOIN users target_user ON tp.target_user_id = target_user.id
            WHERE tp.id = $1
            "#,
            poll_id
        )
        .fetch_one(pool)
        .await?;

        // Count votes
        let vote_counts = sqlx::query!(
            r#"
            SELECT vote, COUNT(*) as count
            FROM poll_votes
            WHERE poll_id = $1
            GROUP BY vote
            "#,
            poll_id
        )
        .fetch_all(pool)
        .await?;

        let mut votes_for = 0;
        let mut votes_against = 0;

        for vc in vote_counts {
            match vc.vote.as_str() {
                "for" => votes_for = vc.count.unwrap_or(0),
                "against" => votes_against = vc.count.unwrap_or(0),
                _ => {}
            }
        }

        // Count total eligible voters
        let eligible_voters = sqlx::query!(
            r#"
            SELECT COUNT(*) as count
            FROM team_members
            WHERE team_id = $1 AND status = 'active' AND user_id != $2
            "#,
            poll_data.team_id,
            poll_data.target_user_id
        )
        .fetch_one(pool)
        .await?;

        let total_eligible = eligible_voters.count.unwrap_or(0);

        // Determine result - need majority (more than 50%) to approve
        let result = if votes_for > total_eligible / 2 {
            PollResult::Approved
        } else if votes_against >= total_eligible / 2 {
            PollResult::Rejected
        } else {
            PollResult::NoConsensus
        };

        // Update poll status
        sqlx::query!(
            "UPDATE team_polls SET status = 'expired', result = $1, executed_at = NOW() WHERE id = $2",
            result.to_string(),
            poll_id
        )
        .execute(pool)
        .await?;

        // If approved, remove the member from the team
        if result == PollResult::Approved {
            sqlx::query!(
                "DELETE FROM team_members WHERE team_id = $1 AND user_id = $2",
                poll_data.team_id,
                poll_data.target_user_id
            )
            .execute(pool)
            .await?;

            tracing::info!("✅ Poll {} expired with approval - removed user {} from team {}",
                poll_id, poll_data.target_username, poll_data.team_name);
        } else {
            tracing::info!("✅ Poll {} expired with result: {:?}", poll_id, result);
        }

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
                tracing::info!("🎮 Running scheduled game management cycle for season '{}' at {}", season_name, now.to_rfc3339());
                
                let manage_games = ManageGameService::new(pool.clone());
                let evaluate_games = GameEvaluationService::new(pool, redis_client);
                
                // Step 1: Run complete game cycle (start due games, finish ended games)
                match manage_games.run_game_cycle().await {
                    Ok((games_ready_to_start, live_games, started_games, finished_games)) => {
                        tracing::info!("✅ [{}] Season '{}' game cycle: {} ready to start, {} live, {} started, {} finished", 
                            now.to_rfc3339(), season_name, games_ready_to_start.len(), live_games.len(), started_games.len(), finished_games.len());

                        if finished_games.len() > 0 {
                            // Step 2: Evaluate any finished games
                            match evaluate_games.evaluate_finished_live_games(&finished_games).await {
                                Ok(result) => {
                                    tracing::info!("✅ Game day completed. Calculated final scores for {} games", result.len());
                                }
                                Err(e) => {
                                    let error_msg = e.to_string();
                                    tracing::error!("❌ Game day evaluation failed for games: {:?} - {}", finished_games, error_msg);
                                }
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
        
        let now = chrono::Utc::now();
        tracing::info!("✅ [{}] Scheduled complete game management cycle for season '{}' with cron: {}", 
            now.to_rfc3339(), season_name_for_logging, cron_expr);
        
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