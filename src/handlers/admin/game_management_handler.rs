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