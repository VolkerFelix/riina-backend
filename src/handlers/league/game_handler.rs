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

#[tracing::instrument(
    name = "Get upcoming games",
    skip(query),
    fields(
        query = %query
    )
)]
/// Get upcoming games
pub async fn get_league_upcoming_games(
    query: web::Query<UpcomingGamesQuery>,
    _pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    // Implementation for upcoming games
    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "data": [],
        "message": "Upcoming games endpoint - implementation needed"
    })))
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