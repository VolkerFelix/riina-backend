use actix_web::{web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;
use serde::Serialize;

use crate::services::WeekGameService;
use crate::middleware::auth::Claims;
use crate::db::live_game_queries::LiveGameQueries;
use crate::models::live_game::LiveScoreEvent;

#[derive(Serialize)]
pub struct LiveGameScore {
    pub game_id: Uuid,
    pub home_team_name: String,
    pub away_team_name: String,
    pub home_score: u32,
    pub away_score: u32,
    pub week_number: i32,
    pub status: String,
}

#[derive(Serialize)]
pub struct LiveScoresResponse {
    pub success: bool,
    pub data: Vec<LiveGameScore>,
    pub total_active_games: usize,
}

#[derive(Serialize)]
pub struct GameManagementResponse {
    pub success: bool,
    pub started_games: Vec<Uuid>,
    pub finished_games: Vec<Uuid>,
    pub message: String,
}

/// Get live scores for all currently active games - now just returns active games without scores
pub async fn get_live_scores(
    pool: web::Data<PgPool>,
    _claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let week_game_service = WeekGameService::new(pool.get_ref().clone());
    
    match week_game_service.get_active_games().await {
        Ok(games) => {
            let game_list: Vec<LiveGameScore> = games
                .into_iter()
                .map(|game| LiveGameScore {
                    game_id: game.id,
                    home_team_name: "TBD".to_string(), // Team names would need separate query
                    away_team_name: "TBD".to_string(),
                    home_score: 0, // No live scoring, just show game is active
                    away_score: 0,
                    week_number: game.week_number,
                    status: game.status.as_str().to_string(),
                })
                .collect();

            let response = LiveScoresResponse {
                success: true,
                total_active_games: game_list.len(),
                data: game_list,
            };

            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to get active games: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Failed to get active games"
            })))
        }
    }
}

/// Get specific game details with actual live scoring data
pub async fn get_game_live_score(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let game_id = path.into_inner();
    
    // First try to get live game data if it exists
    let live_game = sqlx::query!(
        r#"
        SELECT 
            lg.id, lg.home_score, lg.away_score,
            lg.is_active, lg.game_start_time, lg.game_end_time
        FROM live_games lg
        WHERE lg.game_id = $1 AND lg.is_active = true
        ORDER BY lg.created_at DESC
        LIMIT 1
        "#,
        game_id
    )
    .fetch_optional(pool.get_ref())
    .await;

    // Get basic game info
    let game = sqlx::query!(
        r#"
        SELECT 
            lg.id, lg.week_number, lg.status,
            ht.team_name as home_team_name,
            at.team_name as away_team_name
        FROM league_games lg
        JOIN teams ht ON lg.home_team_id = ht.id
        JOIN teams at ON lg.away_team_id = at.id
        WHERE lg.id = $1
        "#,
        game_id
    )
    .fetch_optional(pool.get_ref())
    .await;

    match game {
        Ok(Some(game_data)) => {
            let (home_score, away_score, game_start_time, game_end_time) = 
                if let Ok(Some(ref live_data)) = live_game {
                    // Use live game data if available
                    (live_data.home_score as u32, live_data.away_score as u32, 
                     Some(live_data.game_start_time), Some(live_data.game_end_time))
                } else {
                    // No live game data, return zeros
                    (0, 0, None, None)
                };

            // Fetch scoring events with workout details if we have live game data
            let mut scoring_events: Vec<serde_json::Value> = Vec::new();
            if let Ok(Some(ref live_data)) = live_game {
                let live_game_queries = LiveGameQueries::new(pool.get_ref().clone());
                if let Ok(events) = live_game_queries.get_recent_score_events_with_workout_details(live_data.id, 50).await {
                    scoring_events = events;
                }
            }

            let mut game_info = serde_json::json!({
                "game_id": game_id,
                "home_team_name": game_data.home_team_name,
                "away_team_name": game_data.away_team_name,
                "home_score": home_score,
                "away_score": away_score,
                "week_number": game_data.week_number,
                "status": game_data.status,
                "scoring_events": scoring_events
            });

            // Add optional game timing fields if we have live data
            if let Some(start_time) = game_start_time {
                game_info["game_start_time"] = serde_json::Value::String(start_time.to_rfc3339());
            }
            if let Some(end_time) = game_end_time {
                game_info["game_end_time"] = serde_json::Value::String(end_time.to_rfc3339());
            }

            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "data": game_info
            })))
        }
        Ok(None) => {
            Ok(HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "error": "Game not found"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get game details for {}: {}", game_id, e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Failed to get game details"
            })))
        }
    }
}

/// Admin endpoint to manually trigger game management cycle
pub async fn manage_games(
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let week_game_service = WeekGameService::new(pool.get_ref().clone());
    
    match week_game_service.run_game_cycle().await {
        Ok((pending_games, live_games, started_games, finished_games)) => {
            let message = format!(
                "Pending {} games, live {} games, started {} games, finished {} games", 
                pending_games.len(), 
                live_games.len(), 
                started_games.len(), 
                finished_games.len()
            );
            
            let response = GameManagementResponse {
                success: true,
                started_games,
                finished_games,
                message,
            };

            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to manage games: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Failed to manage games"
            })))
        }
    }
}

/// Get all currently active games
pub async fn get_active_games(
    pool: web::Data<PgPool>,
    _claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let week_game_service = WeekGameService::new(pool.get_ref().clone());
    
    match week_game_service.get_active_games().await {
        Ok(games) => {
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "data": games,
                "total_count": games.len()
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get active games: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Failed to get active games"
            })))
        }
    }
}