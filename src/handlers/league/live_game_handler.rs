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

/// Get specific game details (no live scoring, just game info)
pub async fn get_game_live_score(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let game_id = path.into_inner();
    
    // Just return game details without live scoring
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
            let game_info = LiveGameScore {
                game_id,
                home_team_name: game_data.home_team_name,
                away_team_name: game_data.away_team_name,
                home_score: 0, // No live scoring
                away_score: 0,
                week_number: game_data.week_number,
                status: game_data.status,
            };

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