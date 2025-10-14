use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{Utc, Duration};
use tracing::{info, error};
use std::sync::Arc;

use crate::models::common::ApiResponse;
use crate::services::GameEvaluationService;

#[derive(Debug, Deserialize)]
pub struct StartGamesRequest {
    pub season_id: Uuid,
    pub week_number: Option<i32>,
    pub duration_minutes: Option<i64>, // Duration in minutes (defaults to 10080 for 1 week)
}

#[derive(Debug, sqlx::FromRow)]
struct GameToStart {
    pub id: Uuid,
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
    pub week_number: i32,
}

#[derive(Debug, Serialize)]
pub struct StartGamesResponse {
    pub games_started: i32,
    pub live_games_initialized: i32,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct AdjustLiveGameScoreRequest {
    pub game_id: Uuid,
    pub team_side: String, // "home" or "away"
    pub score_adjustment: i32, // Positive to increase, negative to decrease
    pub reason: String, // Admin reason for the adjustment
}

#[derive(Debug, Serialize)]
pub struct AdjustLiveGameScoreResponse {
    pub game_id: Uuid, // Changed from live_game_id to game_id
    pub previous_scores: (i32, i32), // (home_score, away_score)
    pub new_scores: (i32, i32), // (home_score, away_score)
    pub adjustment_applied: i32, // score_adjustment only (power removed)
    pub message: String,
}

/// POST /admin/games/start-now - Start games immediately for testing
/// Moves specified games to current time and sets them to "in_progress"
pub async fn start_games_now(
    pool: web::Data<PgPool>,
    body: web::Json<StartGamesRequest>,
) -> Result<HttpResponse> {
    info!("Starting games immediately for season {} week {:?}", 
        body.season_id, body.week_number);

    let mut tx = pool.begin().await.map_err(|e| {
        error!("Failed to start transaction: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    // Get games to start
    let games: Vec<GameToStart> = if let Some(week) = body.week_number {
        sqlx::query_as!(
            GameToStart,
            "SELECT id, home_team_id, away_team_id, week_number FROM games WHERE season_id = $1 AND week_number = $2 AND status = 'scheduled' ORDER BY week_number, game_start_time",
            body.season_id,
            week
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to fetch games: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to fetch games")
        })?
    } else {
        sqlx::query_as!(
            GameToStart,
            "SELECT id, home_team_id, away_team_id, week_number FROM games WHERE season_id = $1 AND status = 'scheduled' AND game_start_time > NOW() ORDER BY week_number, game_start_time LIMIT 10",
            body.season_id
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to fetch games: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to fetch games")
        })?
    };

    if games.is_empty() {
        return Ok(HttpResponse::BadRequest().json(
            ApiResponse::<StartGamesResponse>::error("No eligible games found to start")
        ));
    }

    let now = Utc::now();
    
    // Get duration from season if not provided in request
    let duration_seconds = if let Some(duration) = body.duration_minutes {
        duration * 60 // Convert minutes to seconds for backward compatibility
    } else {
        // Get duration from the season configuration
        let season_duration = sqlx::query!(
            "SELECT game_duration_seconds FROM league_seasons WHERE id = $1",
            body.season_id
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to fetch season duration: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to fetch season duration")
        })?;

        match season_duration {
            Some(season) => season.game_duration_seconds,
            None => 518400, // Default: 6 days = 518400 seconds if season not found
        }
    };

    let game_end = now + Duration::seconds(duration_seconds);
    let mut games_started = 0;
    
    info!("Setting game duration to {} seconds ({} hours, {} days)", 
        duration_seconds, 
        duration_seconds / 3600, 
        duration_seconds / (3600 * 24));

    // Update all games to current time and set to in_progress
    for game in &games {
        let result = sqlx::query!(
            r#"
            UPDATE games 
            SET 
                game_start_time = $1,
                game_end_time = $2,
                status = 'in_progress',
                updated_at = NOW()
            WHERE id = $3
            "#,
            now,
            game_end,
            game.id
        )
        .execute(&mut *tx)
        .await;

        match result {
            Ok(_) => {
                games_started += 1;
                info!("Started game {} for week {}", game.id, game.week_number);
            }
            Err(e) => {
                error!("Failed to start game {}: {}", game.id, e);
            }
        }
    }

    // Commit the transaction
    tx.commit().await.map_err(|e| {
        error!("Failed to commit transaction: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    // Start games using the consolidated architecture
    let mut live_games_initialized = 0;

    for game in &games {
        // Games are automatically "live" when set to in_progress status
        // The consolidated architecture handles this in the start_game method
        info!("Game {} is now active with live scoring enabled", game.id);
        live_games_initialized += 1;
    }

    let message = if let Some(week) = body.week_number {
        format!("Started {} games for week {} and initialized {} live games", 
            games_started, week, live_games_initialized)
    } else {
        format!("Started {} upcoming games and initialized {} live games", 
            games_started, live_games_initialized)
    };

    info!("{}", message);

    let response_data = StartGamesResponse {
        games_started,
        live_games_initialized,
        message: message.clone(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(message, response_data)))
}

#[derive(Debug, Serialize)]
pub struct GamesStatusResponse {
    pub season_id: Uuid,
    pub upcoming_games: Vec<GameStatusInfo>,
    pub live_games: Vec<GameStatusInfo>,
    pub finished_games: Vec<GameStatusInfo>,
}

#[derive(Debug, Serialize)]
pub struct GameStatusInfo {
    pub id: Uuid,
    pub week_number: i32,
    pub home_team_name: String,
    pub away_team_name: String,
    pub status: String,
    pub game_start_time: Option<chrono::DateTime<Utc>>,
    pub game_end_time: Option<chrono::DateTime<Utc>>,
    pub has_live_game: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_game_id: Option<Uuid>,
}

/// GET /admin/games/status/{season_id} - Get status of all games in a season
pub async fn get_games_status(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let season_id = path.into_inner();
    
    info!("Getting games status for season {}", season_id);

    // Get all games with their live game status
    let games = sqlx::query!(
        r#"
        SELECT 
            lg.id,
            lg.week_number,
            lg.status,
            lg.game_start_time,
            lg.game_end_time,
            ht.team_name as home_team_name,
            at.team_name as away_team_name,
            NULL::uuid as "live_game_id?" -- No longer needed since games are consolidated
        FROM games lg
        JOIN teams ht ON lg.home_team_id = ht.id
        JOIN teams at ON lg.away_team_id = at.id
        -- No longer need live_games join since games are consolidated
        WHERE lg.season_id = $1
        ORDER BY lg.week_number, lg.game_start_time
        "#,
        season_id
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        error!("Failed to fetch games status: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    let mut upcoming_games = Vec::new();
    let mut live_games = Vec::new();
    let mut finished_games = Vec::new();

    for game in games {
        let game_info = GameStatusInfo {
            id: game.id,
            week_number: game.week_number,
            home_team_name: game.home_team_name,
            away_team_name: game.away_team_name,
            status: game.status.clone(),
            game_start_time: game.game_start_time,
            game_end_time: game.game_end_time,
            has_live_game: game.live_game_id.is_some(),
            live_game_id: game.live_game_id,
        };

        match game.status.as_str() {
            "scheduled" => upcoming_games.push(game_info),
            "in_progress" | "live" => live_games.push(game_info),
            "finished" => finished_games.push(game_info),
            _ => upcoming_games.push(game_info), // Default to upcoming
        }
    }

    let response_data = GamesStatusResponse {
        season_id,
        upcoming_games,
        live_games,
        finished_games,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success("Games status retrieved successfully", response_data)))
}

#[derive(Debug, Deserialize)]
pub struct EvaluateGamesRequest {
    pub date: String, // Date in YYYY-MM-DD format
}

#[derive(Debug, Serialize)]
pub struct EvaluateGamesResponse {
    pub games_evaluated: usize,
    pub games_updated: usize,
    pub success: bool,
    pub message: String,
}

/// POST /admin/games/adjust-score - Manually adjust live game scores
/// Allows admin to directly modify live game scores and power for special circumstances
pub async fn adjust_live_game_score(
    pool: web::Data<PgPool>,
    body: web::Json<AdjustLiveGameScoreRequest>,
    redis_client: Option<web::Data<Arc<redis::Client>>>,
) -> Result<HttpResponse> {
    info!("Admin adjusting game {} score for {} team by {} score. Reason: {}", 
        body.game_id, body.team_side, body.score_adjustment, body.reason);

    // Validate team_side
    if body.team_side != "home" && body.team_side != "away" {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "team_side must be either 'home' or 'away'"
        )));
    }

    // Validate that the game exists and is active
    let game = sqlx::query!(
        r#"
        SELECT g.id, g.home_score, g.away_score, g.status,
               ht.team_name as home_team_name,
               at.team_name as away_team_name
        FROM games g
        JOIN teams ht ON g.home_team_id = ht.id
        JOIN teams at ON g.away_team_id = at.id
        WHERE g.id = $1
        "#,
        body.game_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| {
        error!("Failed to fetch live game: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    let game_data = match game {
        Some(g) => g,
        None => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::<()>::error(
                "Game not found"
            )));
        }
    };

    if game_data.status != "in_progress" {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Cannot adjust scores for game not in progress"
        )));
    }

    let previous_scores = (game_data.home_score, game_data.away_score);

    // Calculate new scores with GREATEST(0, x) to prevent negative values
    let (new_home_score, new_away_score) = if body.team_side == "home" {
        (
            std::cmp::max(0, game_data.home_score + body.score_adjustment),
            game_data.away_score
        )
    } else {
        (
            game_data.home_score,
            std::cmp::max(0, game_data.away_score + body.score_adjustment)
        )
    };

    // Update the game scores
    let updated_game = sqlx::query!(
        r#"
        UPDATE games 
        SET 
            home_score = $1,
            away_score = $2,
            updated_at = NOW()
        WHERE id = $3
        RETURNING home_score, away_score
        "#,
        new_home_score,
        new_away_score,
        body.game_id
    )
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        error!("Failed to update live game scores: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to update scores")
    })?;

    // Log the admin action for audit purposes
    info!("Admin score adjustment completed for game {}: {} vs {} | Scores: {}-{} (was {}-{}) | Reason: {}", 
        body.game_id,
        game_data.home_team_name,
        game_data.away_team_name,
        updated_game.home_score,
        updated_game.away_score,
        previous_scores.0,
        previous_scores.1,
        body.reason
    );

    // Broadcast WebSocket update if Redis is available
    if let Some(redis) = redis_client {
        if let Ok(mut conn) = redis.get_multiplexed_async_connection().await {
            let update_message = serde_json::json!({
                "type": "live_score_update",
                "game_id": body.game_id,
                "home_team_name": game_data.home_team_name,
                "away_team_name": game_data.away_team_name,
                "home_score": updated_game.home_score,
                "away_score": updated_game.away_score,
                "admin_adjustment": true,
                "reason": body.reason
            });
            
            let channel = format!("live_game:{}", body.game_id);
            if let Err(e) = redis::cmd("PUBLISH")
                .arg(&channel)
                .arg(update_message.to_string())
                .query_async::<redis::aio::MultiplexedConnection, ()>(&mut conn)
                .await
            {
                error!("Failed to broadcast admin score adjustment: {}", e);
            } else {
                info!("Broadcasted admin score adjustment to channel: {}", channel);
            }
        }
    }

    let new_scores = (updated_game.home_score, updated_game.away_score);

    let response = AdjustLiveGameScoreResponse {
        game_id: body.game_id,
        previous_scores,
        new_scores,
        adjustment_applied: body.score_adjustment,
        message: format!("Successfully adjusted {} team score by {}", 
                        body.team_side, body.score_adjustment),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response.message.clone(), response)))
}

/// POST /admin/games/trigger-evaluation - Manually trigger game evaluation for all finished games and start upcoming games
pub async fn trigger_game_evaluation(
    pool: web::Data<PgPool>,
    redis_client: Option<web::Data<Arc<redis::Client>>>,
) -> Result<HttpResponse> {
    info!("Manual game evaluation and cycle triggered");

    // Get all finished games regardless of date
    let finished_games = sqlx::query!(
        r#"
        SELECT
            id, season_id, home_team_id, away_team_id,
            week_number, is_first_leg, status as "status: crate::models::league::GameStatus",
            winner_team_id,
            created_at, updated_at,
            home_score, away_score, game_start_time, game_end_time,
            last_score_time, last_scorer_id, last_scorer_name, last_scorer_team
        FROM games
        WHERE status = 'finished'
        ORDER BY game_start_time ASC
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        error!("Failed to fetch finished games: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch games")
    })?;

    let games_evaluated = if !finished_games.is_empty() {
        // Initialize game evaluation service
        let redis_client_inner = match redis_client.as_ref() {
            Some(client) => client.get_ref().clone(),
            None => {
                error!("Redis client not available for game evaluation");
                return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Redis client not available"
                )));
            }
        };

        let evaluation_service = GameEvaluationService::new(
            pool.get_ref().clone(),
            redis_client_inner
        );

        // Convert to the expected format for evaluation
        let games_for_evaluation: Vec<Uuid> = finished_games.into_iter().map(|row| {
            row.id
        }).collect();

        // Evaluate the games
        match evaluation_service.evaluate_finished_live_games(&games_for_evaluation).await {
            Ok(evaluation_results) => {
                info!("Successfully evaluated {} games", evaluation_results.len());
                evaluation_results.len()
            }
            Err(e) => {
                error!("Failed to evaluate games: {}", e);
                return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Game evaluation failed"
                )));
            }
        }
    } else {
        info!("No finished games to evaluate");
        0
    };

    // Now run the game cycle to start upcoming games
    let manage_game_service = crate::services::ManageGameService::new(pool.get_ref().clone());
    let (games_ready_to_start, live_games, started_games, finished_games) =
        match manage_game_service.run_game_cycle().await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to run game cycle: {}", e);
                return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Failed to run game cycle"
                )));
            }
        };

    let message = format!(
        "Evaluation complete: {} games evaluated, {} games started, {} games finished. Currently {} games live, {} games ready to start",
        games_evaluated, started_games.len(), finished_games.len(), live_games.len(), games_ready_to_start.len()
    );

    info!("{}", message);

    let response = serde_json::json!({
        "success": true,
        "games_evaluated": games_evaluated,
        "games_started": started_games.len(),
        "games_finished": finished_games.len(),
        "live_games": live_games.len(),
        "games_ready_to_start": games_ready_to_start.len(),
        "message": message
    });

    Ok(HttpResponse::Ok().json(response))
}

/// POST /admin/games/finish-ongoing - Manually finish all ongoing games
pub async fn finish_ongoing_games(
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    info!("Manual request to finish all ongoing games");

    // Get all in_progress games
    let ongoing_games = sqlx::query!(
        r#"
        SELECT
            id, home_team_id, away_team_id, week_number,
            home_score, away_score
        FROM games
        WHERE status = 'in_progress'
        ORDER BY game_start_time ASC
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        error!("Failed to fetch ongoing games: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch games")
    })?;

    if ongoing_games.is_empty() {
        let message = "No ongoing games to finish";
        info!("{}", message);

        let response = serde_json::json!({
            "success": true,
            "games_finished": 0,
            "message": message
        });
        return Ok(HttpResponse::Ok().json(response));
    }

    let mut games_finished = 0;
    let mut tx = pool.begin().await.map_err(|e| {
        error!("Failed to start transaction: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    // Update all ongoing games to finished status
    for game in &ongoing_games {
        let result = sqlx::query!(
            r#"
            UPDATE games
            SET
                status = 'finished',
                game_end_time = NOW(),
                updated_at = NOW()
            WHERE id = $1 AND status = 'in_progress'
            "#,
            game.id
        )
        .execute(&mut *tx)
        .await;

        match result {
            Ok(_) => {
                games_finished += 1;
                info!("Finished game {} (Week {}, Score: {} - {})",
                    game.id, game.week_number, game.home_score, game.away_score);
            }
            Err(e) => {
                error!("Failed to finish game {}: {}", game.id, e);
            }
        }
    }

    // Commit the transaction
    tx.commit().await.map_err(|e| {
        error!("Failed to commit transaction: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    let message = format!("Successfully finished {} ongoing games", games_finished);
    info!("{}", message);

    let response = serde_json::json!({
        "success": true,
        "games_finished": games_finished,
        "total_ongoing": ongoing_games.len(),
        "message": message
    });

    Ok(HttpResponse::Ok().json(response))
}

/// POST /admin/games/create-summaries - Create game summaries for finished games without summaries
pub async fn create_missing_game_summaries(
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    tracing::info!("Manual request to create game summaries for finished games");

    // Get all finished games without summaries (evaluated games already have summaries)
    let games_without_summaries = sqlx::query!(
        r#"
        SELECT
            g.id, g.season_id, g.home_team_id, g.away_team_id,
            g.week_number, g.is_first_leg, g.status as "status: crate::models::league::GameStatus",
            g.winner_team_id,
            g.created_at, g.updated_at,
            g.home_score, g.away_score, g.game_start_time, g.game_end_time,
            g.last_score_time, g.last_scorer_id, g.last_scorer_name, g.last_scorer_team
        FROM games g
        LEFT JOIN game_summaries gs ON g.id = gs.game_id
        WHERE g.status = 'finished'
        AND gs.id IS NULL
        ORDER BY g.game_start_time ASC
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch games without summaries: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch games")
    })?;

    if games_without_summaries.is_empty() {
        let message = "No games found that need summaries";
        tracing::info!("{}", message);

        let response = serde_json::json!({
            "success": true,
            "summaries_created": 0,
            "message": message
        });
        return Ok(HttpResponse::Ok().json(response));
    }

    let summary_service = crate::services::GameSummaryService::new(pool.get_ref().clone());
    let mut summaries_created = 0;
    let mut errors = Vec::new();

    // Create summaries for each game
    for game_record in &games_without_summaries {
        let game = crate::models::league::LeagueGame {
            id: game_record.id,
            season_id: game_record.season_id,
            home_team_id: game_record.home_team_id,
            away_team_id: game_record.away_team_id,
            week_number: game_record.week_number,
            is_first_leg: game_record.is_first_leg,
            status: game_record.status.clone(),
            winner_team_id: game_record.winner_team_id,
            created_at: game_record.created_at,
            updated_at: game_record.updated_at,
            home_score: game_record.home_score,
            away_score: game_record.away_score,
            game_start_time: game_record.game_start_time,
            game_end_time: game_record.game_end_time,
            last_score_time: game_record.last_score_time,
            last_scorer_id: game_record.last_scorer_id,
            last_scorer_name: game_record.last_scorer_name.clone(),
            last_scorer_team: game_record.last_scorer_team.clone(),
        };

        match summary_service.create_game_summary(&game).await {
            Ok(summary) => {
                summaries_created += 1;
                tracing::info!("✅ Created game summary for game {} (Week {})",
                    game.id, game.week_number);
                tracing::debug!("Summary details: MVP={:?}, LVP={:?}",
                    summary.mvp_username, summary.lvp_username);
            }
            Err(e) => {
                let error_msg = format!("Failed to create summary for game {}: {}", game.id, e);
                tracing::error!("{}", error_msg);
                errors.push(error_msg);
            }
        }
    }

    let message = if errors.is_empty() {
        format!("Successfully created {} game summaries", summaries_created)
    } else {
        format!("Created {} game summaries with {} errors", summaries_created, errors.len())
    };

    tracing::info!("{}", message);

    let response = serde_json::json!({
        "success": errors.is_empty(),
        "summaries_created": summaries_created,
        "total_games_processed": games_without_summaries.len(),
        "errors": errors,
        "message": message
    });

    Ok(HttpResponse::Ok().json(response))
}

/// POST /admin/games/evaluate - Manually trigger game evaluation for a specific date
pub async fn evaluate_games_for_date(
    pool: web::Data<PgPool>,
    redis_client: Option<web::Data<Arc<redis::Client>>>,
    body: web::Json<EvaluateGamesRequest>,
) -> Result<HttpResponse> {
    info!("Manual game evaluation requested for date: {}", body.date);

    // Parse the date
    let evaluation_date = match chrono::NaiveDate::parse_from_str(&body.date, "%Y-%m-%d") {
        Ok(date) => date,
        Err(e) => {
            error!("Invalid date format '{}': {}", body.date, e);
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                "Invalid date format. Use YYYY-MM-DD"
            )));
        }
    };

    // Debug: Check what games exist for this date
    let all_games_debug = sqlx::query!(
        r#"
        SELECT id, status, DATE(game_start_time) as game_date, game_start_time
        FROM games
        WHERE DATE(game_start_time) = $1
        "#,
        evaluation_date
    )
    .fetch_all(pool.get_ref())
    .await
    .unwrap_or_default();

    info!("Debug: Found {} games for date {}", all_games_debug.len(), body.date);
    for game in &all_games_debug {
        info!("Debug: Game {} has status '{}' on date {:?}", game.id, game.status, game.game_date);
    }

    // Get all finished games for the specified date
    let finished_games = sqlx::query!(
        r#"
        SELECT 
            id, season_id, home_team_id, away_team_id,
            week_number, is_first_leg, status as "status: crate::models::league::GameStatus",
            winner_team_id,
            created_at, updated_at,
            home_score, away_score, game_start_time, game_end_time,
            last_score_time, last_scorer_id, last_scorer_name, last_scorer_team
        FROM games
        WHERE status = 'finished'
        AND DATE(game_start_time) = $1
        ORDER BY game_start_time ASC
        "#,
        evaluation_date
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        error!("Failed to fetch finished games for date {}: {}", body.date, e);
        actix_web::error::ErrorInternalServerError("Failed to fetch games")
    })?;

    if finished_games.is_empty() {
        let message = format!("No finished games found for date {}", body.date);
        info!("{}", message);
        
        // Return flattened response to match test expectations
        let response = serde_json::json!({
            "success": true,
            "games_evaluated": 0,
            "games_updated": 0,
            "message": message
        });
        return Ok(HttpResponse::Ok().json(response));
    }

    // Initialize game evaluation service
    let redis_client_inner = match redis_client {
        Some(client) => client.get_ref().clone(),
        None => {
            error!("Redis client not available for game evaluation");
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Redis client not available"
            )));
        }
    };

    let evaluation_service = GameEvaluationService::new(
        pool.get_ref().clone(),
        redis_client_inner
    );

    // Convert to the expected format for evaluation
    let games_for_evaluation: Vec<Uuid> = finished_games.into_iter().map(|row| {
        row.id
    }).collect();

    // Evaluate the games
    match evaluation_service.evaluate_finished_live_games(&games_for_evaluation).await {
        Ok(evaluation_results) => {
            let games_evaluated = evaluation_results.len();
            let message = format!("Successfully evaluated {} games for date {}", games_evaluated, body.date);
            
            info!("{}", message);

            // Return flattened response to match test expectations
            let response = serde_json::json!({
                "success": true,
                "games_evaluated": games_evaluated,
                "games_updated": games_evaluated,
                "message": message
            });

            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            error!("Failed to evaluate games for date {}: {}", body.date, e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Game evaluation failed"
            )))
        }
    }
}