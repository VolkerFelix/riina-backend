use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{Utc, Duration};
use tracing::{info, error};
use std::sync::Arc;

use crate::models::common::ApiResponse;
use crate::services::{LiveGameService, GameEvaluationService};

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
            "SELECT id, home_team_id, away_team_id, week_number FROM league_games WHERE season_id = $1 AND week_number = $2 AND status = 'scheduled' ORDER BY week_number, scheduled_time",
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
            "SELECT id, home_team_id, away_team_id, week_number FROM league_games WHERE season_id = $1 AND status = 'scheduled' AND scheduled_time > NOW() ORDER BY week_number, scheduled_time LIMIT 10",
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
    let duration_minutes = if let Some(duration) = body.duration_minutes {
        duration
    } else {
        // Get duration from the season configuration
        let season_duration = sqlx::query!(
            "SELECT game_duration_minutes FROM league_seasons WHERE id = $1",
            body.season_id
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to fetch season duration: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to fetch season duration")
        })?;

        match season_duration {
            Some(season) => season.game_duration_minutes as i64,
            None => 8640, // Default: 6 days = 8640 minutes if season not found
        }
    };

    let game_end = now + Duration::minutes(duration_minutes);
    let mut games_started = 0;
    
    info!("Setting game duration to {} minutes ({} hours, {} days)", 
        duration_minutes, 
        duration_minutes / 60, 
        duration_minutes / (60 * 24));

    // Update all games to current time and set to in_progress
    for game in &games {
        let result = sqlx::query!(
            r#"
            UPDATE league_games 
            SET 
                scheduled_time = $1,
                week_start_date = $1,
                week_end_date = $2,
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

    // Initialize live games for the started games
    let live_game_service = LiveGameService::new(pool.get_ref().clone(), None);
    let mut live_games_initialized = 0;

    for game in &games {
        match live_game_service.initialize_live_game(game.id).await {
            Ok(_) => {
                live_games_initialized += 1;
                info!("Initialized live game for {}", game.id);
            }
            Err(e) => {
                error!("Failed to initialize live game for {}: {}", game.id, e);
            }
        }
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
    pub scheduled_time: chrono::DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub week_end_date: Option<chrono::DateTime<Utc>>,
    pub has_live_game: bool,
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
            lg.scheduled_time,
            lg.week_end_date as "week_end_date?",
            ht.team_name as home_team_name,
            at.team_name as away_team_name,
            live_g.id as "live_game_id?"
        FROM league_games lg
        JOIN teams ht ON lg.home_team_id = ht.id
        JOIN teams at ON lg.away_team_id = at.id
        LEFT JOIN live_games live_g ON lg.id = live_g.game_id AND live_g.is_active = true
        WHERE lg.season_id = $1
        ORDER BY lg.week_number, lg.scheduled_time
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
            scheduled_time: game.scheduled_time,
            week_end_date: game.week_end_date,
            has_live_game: game.live_game_id.is_some(),
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

/// POST /admin/games/evaluate - Evaluate finished games for a specific date
pub async fn evaluate_games_for_date(
    pool: web::Data<PgPool>,
    body: web::Json<EvaluateGamesRequest>,
    redis: Option<web::Data<Arc<redis::Client>>>,
) -> Result<HttpResponse> {
    info!("Evaluating games for date: {}", body.date);

    // Parse the date
    let date = chrono::NaiveDate::parse_from_str(&body.date, "%Y-%m-%d")
        .map_err(|e| {
            error!("Invalid date format: {}", e);
            actix_web::error::ErrorBadRequest("Invalid date format. Use YYYY-MM-DD")
        })?;

    // Get all finished games for the specified date
    let finished_games = sqlx::query!(
        r#"
        SELECT id
        FROM league_games
        WHERE DATE(scheduled_time) = $1
        AND status = 'finished'
        AND (home_score_final IS NULL OR away_score_final IS NULL)
        "#,
        date
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        error!("Failed to fetch finished games: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    if finished_games.is_empty() {
        return Ok(HttpResponse::Ok().json(EvaluateGamesResponse {
            games_evaluated: 0,
            games_updated: 0,
            success: true,
            message: format!("No finished games found for date {}", date),
        }));
    }

    let game_ids: Vec<uuid::Uuid> = finished_games.iter().map(|g| g.id).collect();
    
    // Create the evaluation service with Redis for WebSocket notifications
    let evaluation_service = GameEvaluationService::new_with_redis(
        pool.get_ref().clone(), 
        redis.map(|r| r.get_ref().clone())
    );
    
    match evaluation_service.evaluate_finished_live_games(game_ids).await {
        Ok(results) => {
            let games_updated = results.len();
            let message = format!("Successfully evaluated {} games for date {}", games_updated, date);
            
            info!("{}", message);
            
            Ok(HttpResponse::Ok().json(EvaluateGamesResponse {
                games_evaluated: games_updated,
                games_updated,
                success: true,
                message,
            }))
        }
        Err(e) => {
            let error_msg = format!("Failed to evaluate games: {}", e);
            error!("{}", error_msg);
            
            Ok(HttpResponse::InternalServerError().json(EvaluateGamesResponse {
                games_evaluated: 0,
                games_updated: 0,
                success: false,
                message: error_msg,
            }))
        }
    }
}