use actix_web::{web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;
use serde::Serialize;
use std::sync::Arc;

use crate::services::ManageGameService;
use crate::middleware::auth::Claims;
// Removed unused import: use crate::db::game_queries::GameQueries;

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
    _redis_client: web::Data<Arc<redis::Client>>,
    _claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let week_game_service = ManageGameService::new(pool.get_ref().clone());
    
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
    
    // Get game info with live scoring data from unified games table
    let game = sqlx::query!(
        r#"
        SELECT 
            g.id, g.week_number, g.status,
            g.home_score, g.away_score,
            g.game_start_time, g.game_end_time,
            ht.team_name as home_team_name,
            at.team_name as away_team_name
        FROM games g
        JOIN teams ht ON g.home_team_id = ht.id
        JOIN teams at ON g.away_team_id = at.id
        WHERE g.id = $1
        "#,
        game_id
    )
    .fetch_optional(pool.get_ref())
    .await;

    match game {
        Ok(Some(game_data)) => {
            let home_score = game_data.home_score as u32;
            let away_score = game_data.away_score as u32;
            
            // Fetch scoring events from live_score_events table
            let scoring_events = sqlx::query!(
                r#"
                SELECT 
                    lse.id, lse.user_id, lse.score_points,
                    lse.occurred_at, lse.event_type::text as "event_type!", lse.description,
                    lse.username, lse.workout_data_id
                FROM live_score_events lse
                WHERE lse.game_id = $1
                ORDER BY lse.occurred_at DESC
                LIMIT 50
                "#,
                game_id
            )
            .fetch_all(pool.get_ref())
            .await
            .unwrap_or_else(|_| vec![]);

            let scoring_events_json: Vec<serde_json::Value> = scoring_events
                .into_iter()
                .map(|event| {
                    serde_json::json!({
                        "id": event.id,
                        "user_id": event.user_id,
                        "username": event.username,
                        "score_points": event.score_points,
                        "occurred_at": event.occurred_at,
                        "event_type": event.event_type.to_string(),
                        "description": event.description,
                        "workout_data_id": event.workout_data_id
                    })
                })
                .collect();

            let mut game_info = serde_json::json!({
                "game_id": game_id,
                "home_team_name": game_data.home_team_name,
                "away_team_name": game_data.away_team_name,
                "home_score": home_score,
                "away_score": away_score,
                "week_number": game_data.week_number,
                "status": game_data.status,
                "scoring_events": scoring_events_json
            });

            // Add optional game timing fields
            if let Some(start_time) = game_data.game_start_time {
                game_info["game_start_time"] = serde_json::Value::String(start_time.to_rfc3339());
            }
            if let Some(end_time) = game_data.game_end_time {
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
    _redis_client: web::Data<Arc<redis::Client>>,
) -> Result<HttpResponse> {
    let week_game_service = ManageGameService::new(pool.get_ref().clone());
    
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
    _redis_client: web::Data<Arc<redis::Client>>,
    _claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let week_game_service = ManageGameService::new(pool.get_ref().clone());
    
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