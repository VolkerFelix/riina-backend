use actix_web::{web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;
use serde_json::json;

use crate::league::league::LeagueService;
use crate::middleware::auth::Claims;
use crate::models::league::*;
use crate::services::game_summary_service::GameSummaryService;

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
    name = "Get live games",
    skip(query, pool),
    fields(
        season_id = ?query.season_id,
        limit = ?query.limit
    )
)]
pub async fn get_league_live_games(
    query: web::Query<UpcomingGamesQuery>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    tracing::info!("Getting live games for season: {:?}, limit: {:?}", 
        query.season_id, query.limit);

    let league_service = LeagueService::new(pool.get_ref().clone());
    
    match league_service.get_live_games(query.season_id, query.limit).await {
        Ok(games) => {
            tracing::info!("Successfully retrieved {} live games", games.len());
            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "data": games,
                "total_count": games.len()
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get live games: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to retrieve live games"
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

/// Get game summary
#[tracing::instrument(
    name = "Get game summary",
    skip(pool),
    fields(
        game_id = %game_id
    )
)]
pub async fn get_game_summary(
    game_id: web::Path<Uuid>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    tracing::info!("Getting game summary for game: {}", game_id);

    let summary_service = GameSummaryService::new(pool.get_ref().clone());

    match summary_service.get_game_summary(*game_id).await {
        Ok(Some(summary)) => {
            // Get team names from the database
            let game = sqlx::query!(
                r#"
                SELECT
                    g.id,
                    ht.team_name as home_team_name,
                    at.team_name as away_team_name
                FROM games g
                JOIN teams ht ON g.home_team_id = ht.id
                JOIN teams at ON g.away_team_id = at.id
                WHERE g.id = $1
                "#,
                *game_id
            )
            .fetch_optional(pool.get_ref())
            .await;

            match game {
                Ok(Some(game_data)) => {
                    let response = GameSummaryResponse {
                        summary,
                        home_team_name: game_data.home_team_name,
                        away_team_name: game_data.away_team_name,
                    };

                    Ok(HttpResponse::Ok().json(json!({
                        "success": true,
                        "data": response
                    })))
                }
                Ok(None) => {
                    tracing::warn!("Game {} not found", game_id);
                    Ok(HttpResponse::NotFound().json(json!({
                        "success": false,
                        "message": "Game not found"
                    })))
                }
                Err(e) => {
                    tracing::error!("Failed to get game details: {}", e);
                    Ok(HttpResponse::InternalServerError().json(json!({
                        "success": false,
                        "message": "Failed to retrieve game details"
                    })))
                }
            }
        }
        Ok(None) => {
            tracing::warn!("Game summary not found for game: {}", game_id);
            Ok(HttpResponse::NotFound().json(json!({
                "success": false,
                "message": "Game summary not found. The game may not have been evaluated yet."
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get game summary: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to retrieve game summary"
            })))
        }
    }
}