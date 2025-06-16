use actix_web::{web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;
use serde_json::json;

use crate::league::league::LeagueService;
use crate::middleware::auth::Claims;
use crate::models::league::*;

/// Update game result
#[tracing::instrument(
    name = "Update game result",
    skip(result_request, pool, claims),
    fields(
        game_id = %game_id,
        admin_user = %claims.username
    )
)]
pub async fn update_league_game_result(
    game_id: Uuid,
    result_request: web::Json<GameResultRequest>,
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    tracing::info!("Updating game {} result: {} - {} by admin: {}", 
        game_id, result_request.home_score, result_request.away_score, claims.username);

    let league_service = LeagueService::new(pool.get_ref().clone());
    
    match league_service.update_game_result(
        game_id,
        result_request.home_score,
        result_request.away_score,
    ).await {
        Ok(()) => {
            tracing::info!("Successfully updated game {} result", game_id);
            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "message": "Game result updated successfully"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to update game {} result: {}", game_id, e);
            Ok(HttpResponse::BadRequest().json(json!({
                "success": false,
                "message": format!("Failed to update game result: {}", e)
            })))
        }
    }
}

/// Get upcoming games
#[tracing::instrument(
    name = "Get upcoming games",
    skip(query, pool),
    fields(
        season_id = ?query.season_id,
        limit = ?query.limit
    )
)]
pub async fn get_league_upcoming_games(
    query: web::Query<UpcomingGamesQuery>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    tracing::info!("Getting upcoming games for season: {:?}, limit: {:?}", 
        query.season_id, query.limit);

    let league_service = LeagueService::new(pool.get_ref().clone());
    
    match league_service.get_upcoming_games(query.season_id, query.limit).await {
        Ok(games) => {
            tracing::info!("Successfully retrieved {} upcoming games", games.len());
            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "data": games,
                "total_count": games.len()
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get upcoming games: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to retrieve upcoming games"
            })))
        }
    }
}

#[tracing::instrument(
    name = "Get game countdown",
    skip(query, pool),
    fields(
        query = %query
    )
)]
/// Get countdown information
pub async fn get_game_countdown(
    query: web::Query<CountdownQuery>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let league_service = LeagueService::new(pool.get_ref().clone());
    
    match league_service.get_countdown_info(query.season_id).await {
        Ok(countdown_info) => {
            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "data": countdown_info
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get countdown info: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to get countdown information"
            })))
        }
    }
}