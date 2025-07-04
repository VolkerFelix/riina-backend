use actix_web::{web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::services::WeekGameService;
use crate::middleware::auth::Claims;

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

/// Get live scores for all currently active games
pub async fn get_live_scores(
    pool: web::Data<PgPool>,
    _claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let week_game_service = WeekGameService::new(pool.get_ref().clone());
    
    match week_game_service.get_live_scores().await {
        Ok(live_scores) => {
            let games: Vec<LiveGameScore> = live_scores
                .into_iter()
                .map(|(game_id, stats)| LiveGameScore {
                    game_id,
                    home_team_name: stats.home_team_name,
                    away_team_name: stats.away_team_name,
                    home_score: stats.home_score,
                    away_score: stats.away_score,
                    week_number: 0, // This would need to be fetched separately
                    status: "in_progress".to_string(),
                })
                .collect();

            let response = LiveScoresResponse {
                success: true,
                total_active_games: games.len(),
                data: games,
            };

            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to get live scores: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Failed to get live scores"
            })))
        }
    }
}

/// Get live score for a specific game
pub async fn get_game_live_score(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let game_id = path.into_inner();
    let week_game_service = WeekGameService::new(pool.get_ref().clone());
    
    match week_game_service.get_game_live_score(game_id).await {
        Ok(Some(stats)) => {
            let game_score = LiveGameScore {
                game_id: stats.game_id,
                home_team_name: stats.home_team_name,
                away_team_name: stats.away_team_name,
                home_score: stats.home_score,
                away_score: stats.away_score,
                week_number: 0, // This would need to be fetched separately
                status: "in_progress".to_string(),
            };

            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "data": game_score
            })))
        }
        Ok(None) => {
            Ok(HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "error": "Game not found or not currently active"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get game live score for {}: {}", game_id, e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Failed to get game live score"
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
        Ok((started_games, finished_games)) => {
            let message = format!(
                "Started {} games, finished {} games", 
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